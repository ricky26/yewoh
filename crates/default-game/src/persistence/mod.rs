use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};

use bevy_app::{App, IntoSystemAppConfig, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::query::{ReadOnlyWorldQuery, WorldQuery};
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::system::{Deferred, Query, Resource, SystemBuffer, SystemMeta, SystemParam};
use bevy_ecs::world::{FromWorld, Mut, World};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::Error as DError;
use serde::ser::SerializeStruct;
use sqlx::{Database, Pool};
use sqlx::migrate::Migrate;

use de::{BundleValuesVisitor, WorldVisitor};
pub use hierarchy::{ChangePersistence, PersistenceCommandsExt, set_persistent};
use ser::{BufferBundlesSerializer, BufferSerializer, ErasedOk};

use crate::persistence::prefab::PrefabSerializer;

mod ser;
mod de;
mod hierarchy;
pub mod prefab;
pub mod entity;
pub mod db;

pub async fn migrate<D: Database>(db: &Pool<D>) -> anyhow::Result<()>
    where <D as Database>::Connection: Migrate {
    sqlx::migrate!("./migrations")
        .run(db)
        .await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntityReference(u32);

#[derive(Default)]
pub struct SerializeContextInner {
    next_entity_id: u32,
    entity_map: HashMap<Entity, EntityReference>,
}

impl SerializeContextInner {
    pub fn map_entity(&mut self, entity: Entity) -> EntityReference {
        *self.entity_map.entry(entity)
            .or_insert_with(|| {
                self.next_entity_id += 1;
                EntityReference(self.next_entity_id)
            })
    }
}

#[derive(Default)]
pub struct SerializeContext {
    inner: RefCell<SerializeContextInner>,
}

impl SerializeContext {
    pub fn map_entity(&self, entity: Entity) -> EntityReference {
        self.inner.borrow_mut().map_entity(entity)
    }
}

pub struct DeserializeContext<'w> {
    entity_map: HashMap<EntityReference, Entity>,
    world: &'w mut World,
}

impl<'w> DeserializeContext<'w> {
    pub fn world_mut(&mut self) -> &mut World {
        self.world
    }

    pub fn map_entity(&mut self, reference: EntityReference) -> Entity {
        *self.entity_map.entry(reference)
            .or_insert_with(|| self.world.spawn_empty().id())
    }
}

#[derive(ScheduleLabel, Hash, Debug, Clone, PartialEq, Eq)]
pub struct SerializeSchedule;

pub trait BundleSerializer: FromWorld + Send + 'static {
    type Query: ReadOnlyWorldQuery;
    type Filter: ReadOnlyWorldQuery;
    type Bundle: Send + Sync + 'static;

    fn id() -> &'static str;
    fn priority() -> i32 { 0 }
    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle;
    fn serialize<S: Serializer>(ctx: &SerializeContext, s: S, bundle: &Self::Bundle) -> Result<S::Ok, S::Error>;
    fn deserialize<'de, D: Deserializer<'de>>(ctx: &mut DeserializeContext, d: D, entity: Entity) -> Result<(), D::Error>;
}

type BundleDeserializer = fn(ctx: &mut DeserializeContext, d: &mut dyn erased_serde::Deserializer) -> Result<(), erased_serde::Error>;

pub struct SerializedBuffer {
    serializer_id: String,
    priority: i32,
    serialize: Box<dyn (Fn(&SerializeContext, &mut dyn erased_serde::Serializer) -> Result<ErasedOk, erased_serde::Error>) + Send + Sync + 'static>,
}

impl PartialEq<Self> for SerializedBuffer {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Eq for SerializedBuffer {}

impl PartialOrd for SerializedBuffer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.priority.partial_cmp(&other.priority)
    }
}

impl Ord for SerializedBuffer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

#[derive(Resource, Default)]
pub struct SerializedBuffers {
    buffers: VecDeque<SerializedBuffer>,
}

impl SerializedBuffers {
    pub fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut world = s.serialize_struct("World", 1)?;
        let ctx = &SerializeContext::default();
        world.serialize_field("bundles", &BufferBundlesSerializer::new(ctx, &self.buffers))?;
        world.end()
    }
}

pub struct SerializedBundlesBuffer<T: BundleSerializer> {
    items: Vec<(Entity, T::Bundle)>,
}

impl<T: BundleSerializer> FromWorld for SerializedBundlesBuffer<T> {
    fn from_world(_world: &mut World) -> Self {
        Self {
            items: Vec::new(),
        }
    }
}

impl<T: BundleSerializer> SystemBuffer for SerializedBundlesBuffer<T> {
    fn apply(&mut self, _system_meta: &SystemMeta, world: &mut World) {
        let items = std::mem::take(&mut self.items);
        if items.is_empty() {
            return;
        }

        let serialize = move |ctx: &SerializeContext, s: &mut dyn erased_serde::Serializer|
            erased_serde::serialize(&BufferSerializer::<T>::new(ctx, &items), s);
        let buffer = SerializedBuffer {
            serializer_id: T::id().to_string(),
            priority: T::priority(),
            serialize: Box::new(serialize),
        };
        let buffers = &mut world.resource_mut::<SerializedBuffers>()
            .buffers;
        let index = match buffers.binary_search_by(|i|
            i.priority.cmp(&buffer.priority)
            .then(Ordering::Greater)) {
            Ok(x) => x,
            Err(x) => x,
        };
        buffers.insert(index, buffer);
    }
}

#[derive(SystemParam)]
pub struct SerializedBundles<'s, T: BundleSerializer> {
    deferred: Deferred<'s, SerializedBundlesBuffer<T>>,
}

impl<'w, T: BundleSerializer> SerializedBundles<'w, T> {
    pub fn push(&mut self, entity: Entity, bundle: T::Bundle) {
        self.deferred.items.push((entity, bundle));
    }

    pub fn extend(&mut self, bundles: impl Iterator<Item=(Entity, T::Bundle)>) {
        self.deferred.items.extend(bundles);
    }
}

fn extract_bundles<T: BundleSerializer>(
    query: Query<(Entity, T::Query), T::Filter>,
    mut bundles: SerializedBundles<T>,
) {
    bundles.extend(query.iter()
        .map(|(entity, item)| (entity, T::extract(item))));
}

fn deserialize_bundles<T: BundleSerializer>(ctx: &mut DeserializeContext, d: &mut dyn erased_serde::Deserializer) -> Result<(), erased_serde::Error> {
    d.deserialize_seq(BundleValuesVisitor::<T>::new(ctx))
}

#[derive(Default, Resource)]
pub struct BundleSerializers {
    deserializers: HashMap<String, BundleDeserializer>,
}

impl BundleSerializers {
    pub fn insert<T: BundleSerializer>(&mut self) {
        self.deserializers.insert(T::id().to_string(), deserialize_bundles::<T>);
    }

    pub fn deserialize_bundle_values<'de, D: Deserializer<'de>>(&self, ctx: &mut DeserializeContext, id: &str, d: D) -> Result<(), D::Error> {
        if let Some(deserialize) = self.deserializers.get(id) {
            let mut d = <dyn erased_serde::Deserializer>::erase(d);
            (deserialize)(ctx, &mut d)
                .map_err(|e| D::Error::custom(e))
        } else {
            Err(D::Error::custom(format!("unknown bundle ID {id}")))
        }
    }

    pub fn deserialize_into_world<'de, D: Deserializer<'de>>(&self, world: &mut World, d: D) -> Result<(), D::Error> {
        let mut ctx = DeserializeContext {
            entity_map: Default::default(),
            world,
        };

        d.deserialize_struct("World", &["bundles"], WorldVisitor {
            ctx: &mut ctx,
            deserializers: self,
        })
    }
}

pub trait SerializationWorldExt {
    fn deserialize<'de, D: Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error>;
    fn serialize(&mut self) -> SerializedBuffers;
}

impl SerializationWorldExt for World {
    fn deserialize<'de, D: Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        self.resource_scope(|world, serializers: Mut<BundleSerializers>|
            serializers.deserialize_into_world(world, d))
    }

    fn serialize(&mut self) -> SerializedBuffers {
        self.insert_resource(SerializedBuffers::default());
        self.run_schedule(SerializeSchedule);
        self.remove_resource::<SerializedBuffers>().unwrap()
    }
}

pub trait SerializationSetupExt {
    fn register_serializer<T: BundleSerializer>(&mut self) -> &mut Self;
}

impl SerializationSetupExt for App {
    fn register_serializer<T: BundleSerializer>(&mut self) -> &mut Self {
        self.world.init_resource::<BundleSerializers>();
        self.world.resource_mut::<BundleSerializers>().insert::<T>();
        self
            .add_system(extract_bundles::<T>.in_schedule(SerializeSchedule))
    }
}

#[derive(Default)]
pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_schedule(SerializeSchedule)
            .init_resource::<BundleSerializers>()
            .register_serializer::<PrefabSerializer>();
    }
}
