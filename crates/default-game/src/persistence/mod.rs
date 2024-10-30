use std::any::TypeId;
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};

use bevy::app::{App, Plugin};
use bevy::ecs::entity::{Entity, EntityHashMap};
use bevy::ecs::query::{QueryFilter, ReadOnlyQueryData, WorldQuery};
use bevy::ecs::reflect::ReflectMapEntities;
use bevy::ecs::schedule::ScheduleLabel;
use bevy::ecs::system::{Deferred, Query, Resource, SystemBuffer, SystemMeta, SystemParam};
use bevy::ecs::world::{FromWorld, Mut, World};
use bevy::prelude::{AppTypeRegistry, EntityMapper, FromReflect};
use bevy::reflect::{GetTypeRegistration, PartialReflect, TypeRegistry, Typed};
use de::{BundleValuesVisitor, WorldVisitor};
use ser::{BufferBundlesSerializer, BufferSerializer};
use serde::de::Error as DError;
use serde::ser::SerializeStruct;
use serde::{Deserializer, Serializer};
use sqlx::migrate::Migrate;
use sqlx::{Database, Pool};
use tracing::error;

use crate::persistence::prefab::PrefabSerializer;

mod ser;
mod de;
pub mod prefab;
pub mod db;

pub async fn migrate<D: Database>(db: &Pool<D>) -> anyhow::Result<()>
where
    <D as Database>::Connection: Migrate,
{
    sqlx::migrate!("./migrations")
        .run(db)
        .await?;
    Ok(())
}

pub struct SerializeContext<'a> {
    type_registry: &'a TypeRegistry,
}

pub struct DeserializeContext<'a> {
    type_registry: &'a TypeRegistry,
}

#[derive(ScheduleLabel, Hash, Debug, Clone, PartialEq, Eq)]
pub struct SerializeSchedule;

pub trait BundleSerializer: FromWorld + Send + 'static {
    type Query: ReadOnlyQueryData;
    type Filter: QueryFilter;
    type Bundle: Typed + FromReflect + GetTypeRegistration;

    fn id() -> &'static str;
    fn priority() -> i32 { 0 }
    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle;
    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle);
}

type BundleDeserializer = fn(ctx: &mut DeserializeContext, d: &mut dyn erased_serde::Deserializer) -> Result<Box<dyn PartialReflect>, erased_serde::Error>;
type BundleMapper = fn(type_registry: &TypeRegistry, mapper: &mut dyn EntityMapper, bundles: &mut dyn PartialReflect);
type BundleSpawner = fn(world: &mut World, bundles: Box<dyn PartialReflect>);


pub struct SerializedBuffer {
    serializer_id: String,
    priority: i32,
    serialize: Box<dyn Fn(&mut dyn FnMut(&dyn erased_serde::Serialize)) + Send + Sync + 'static>,
}

impl PartialEq<Self> for SerializedBuffer {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Eq for SerializedBuffer {}

impl PartialOrd for SerializedBuffer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
        world.serialize_field("bundles", &BufferBundlesSerializer::new(&self.buffers))?;
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

        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let serialize = move |callback: &mut dyn FnMut(&dyn erased_serde::Serialize)| {
            let type_registry = type_registry.read();
            let ctx = SerializeContext {
                type_registry: &type_registry,
            };
            callback(&BufferSerializer::<T>::new(&ctx, &items));
        };
        let buffer = SerializedBuffer {
            serializer_id: T::id().to_string(),
            priority: T::priority(),
            serialize: Box::new(serialize),
        };
        let buffers = &mut world.resource_mut::<SerializedBuffers>()
            .buffers;
        let index = buffers.binary_search_by(|i|
            i.priority.cmp(&buffer.priority).then(Ordering::Greater))
            .unwrap_or_else(|x| x);
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

struct NewEntityMapper<'a> {
    entity_map: &'a mut EntityHashMap<Entity>,
    world: &'a mut World,
}

impl EntityMapper for NewEntityMapper<'_> {
    fn map_entity(&mut self, entity: Entity) -> Entity {
        if let Some(existing) = self.entity_map.get(&entity) {
            *existing
        } else {
            let new_entity = self.world.spawn_empty().id();
            self.entity_map.insert(entity, new_entity);
            new_entity
        }
    }
}

fn extract_bundles<T: BundleSerializer>(
    query: Query<(Entity, T::Query), T::Filter>,
    mut bundles: SerializedBundles<T>,
) {
    bundles.extend(query.iter()
        .map(|(entity, item)| (entity, T::extract(item))));
}

fn deserialize_bundles<T: BundleSerializer>(ctx: &mut DeserializeContext, d: &mut dyn erased_serde::Deserializer) -> Result<Box<dyn PartialReflect>, erased_serde::Error> {
    d.deserialize_seq(BundleValuesVisitor::<T>::new(ctx))
}

fn map_bundles<T: BundleSerializer>(
    type_registry: &TypeRegistry, mapper: &mut dyn EntityMapper, bundles: &mut dyn PartialReflect,
) {
    let bundles = bundles.try_downcast_mut::<Vec<(Entity, T::Bundle)>>().unwrap();
    let registration = type_registry.get(TypeId::of::<T::Bundle>())
        .expect("bundle type not registered");
    let map_entities = registration.data::<ReflectMapEntities>();

    for (entity, ref mut bundle) in bundles {
        *entity = mapper.map_entity(*entity);

        if let Some(map_entities) = &map_entities {
            map_entities.map_entities(bundle, mapper);
        }
    }
}

fn spawn_bundles<T: BundleSerializer>(
    world: &mut World, bundles: Box<dyn PartialReflect>,
) {
    let bundles = *bundles.try_downcast::<Vec<(Entity, T::Bundle)>>().unwrap();

    for (entity, bundle) in bundles {
        T::insert(world, entity, bundle);
    }
}

struct BundleOps {
    deserialize: BundleDeserializer,
    map_entities: BundleMapper,
    spawn: BundleSpawner,
}

#[derive(Default, Resource)]
pub struct BundleSerializers {
    ops: HashMap<String, BundleOps>,
}

impl BundleSerializers {
    pub fn insert<T: BundleSerializer>(&mut self) {
        self.ops.insert(T::id().to_string(), BundleOps {
            deserialize: deserialize_bundles::<T>,
            map_entities: map_bundles::<T>,
            spawn: spawn_bundles::<T>,
        });
    }

    fn deserialize_bundle_values<'de, D: Deserializer<'de>>(
        &self, ctx: &mut DeserializeContext, id: &str, d: D,
    ) -> Result<Box<dyn PartialReflect>, D::Error> {
        if let Some(ops) = self.ops.get(id) {
            let mut d = <dyn erased_serde::Deserializer>::erase(d);
            (ops.deserialize)(ctx, &mut d)
                .map_err(D::Error::custom)
        } else {
            Err(D::Error::custom(format!("unknown bundle ID {id}")))
        }
    }

    pub fn deserialize_into_world<'de, D: Deserializer<'de>>(&self, world: &mut World, d: D) -> Result<(), D::Error> {
        let mut bundles = {
            let type_registry = world.resource::<AppTypeRegistry>().read();
            let mut ctx = DeserializeContext {
                type_registry: &type_registry,
            };

             d.deserialize_struct("World", &["bundles"], WorldVisitor {
                ctx: &mut ctx,
                deserializers: self,
            })?
        };

        let mut entity_map = EntityHashMap::default();
        {
            let mut mapper = NewEntityMapper { entity_map: &mut entity_map, world };
            let type_registry = mapper.world.resource::<AppTypeRegistry>().clone();
            let type_registry = type_registry.read();

            for (id, bundles) in &mut bundles {
                match self.ops.get(id) {
                    Some(ops) => (ops.map_entities)(&type_registry, &mut mapper, bundles.as_mut()),
                    None => {
                        error!("unknown bundle type {id}");
                    }
                }
            }
        }

        for (id, bundles) in bundles {
            match self.ops.get(&id) {
                Some(ops) => (ops.spawn)(world, bundles),
                None => {
                    error!("unknown bundle type {id}");
                }
            }
        }

        Ok(())
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
        self.world_mut().init_resource::<BundleSerializers>();
        self.world_mut().resource_mut::<BundleSerializers>().insert::<T>();
        self
            .register_type::<T::Bundle>()
            .add_systems(SerializeSchedule, extract_bundles::<T>)
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
