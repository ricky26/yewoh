use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use bevy_app::{App, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Command, EntityCommands, Resource};
use bevy_ecs::world::World;
use serde::{Deserialize, Deserializer};
use serde::de::{DeserializeSeed, Error, MapAccess, Visitor};
use tokio::fs;

use crate::data::prefab::common::LocationPrefab;
use crate::data::prefab::inheritance::InheritancePrefab;

pub mod inheritance;

pub mod common;

pub trait PrefabBundle: Send + Sync {
    fn write(&self, prefab: &Prefab, world: &mut World, entity: Entity);
}

pub trait FromPrefabTemplate: PrefabBundle + 'static {
    type Template: for<'de> Deserialize<'de> + 'static;
    fn from_template(template: Self::Template) -> Self;
}

#[derive(Clone)]
pub struct EntityPrefab {
    bundles: Vec<Arc<dyn PrefabBundle>>,
}

impl Debug for EntityPrefab {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntityPrefab {{ {} bundles }}", self.bundles.len())
    }
}

#[derive(Debug, Clone)]
pub struct Prefab {
    entities: HashMap<String, EntityPrefab>,
}

impl Prefab {
    pub fn from_single_entity(entity: EntityPrefab) -> Prefab {
        let mut entities = HashMap::new();
        entities.insert("".into(), entity);
        Self { entities }
    }

    pub fn from_entities(entities: HashMap<String, EntityPrefab>) -> Prefab {
        Self { entities }
    }

    fn write_entity_internal(&self, world: &mut World, entity: Entity, prefab: &EntityPrefab) {
        for bundle in &prefab.bundles {
            bundle.write(self, world, entity);
        }
    }

    pub fn write_entity(&self, world: &mut World, entity: Entity, id: &str) {
        if let Some(prefab) = self.entities.get(id) {
            self.write_entity_internal(world, entity, prefab);
        }
    }

    pub fn write(&self, world: &mut World, entity: Entity) {
        self.write_entity(world, entity, "")
    }
}

pub struct InsertPrefab {
    pub entity: Entity,
    pub prefab: Arc<Prefab>,
}

impl Command for InsertPrefab {
    fn write(self, world: &mut World) {
        self.prefab.write(world, self.entity);
    }
}

struct PrefabVisitor<'a> {
    factory: &'a PrefabFactory,
}

impl<'de, 'a> Visitor<'de> for PrefabVisitor<'a> {
    type Value = Document;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "prefab struct")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        let mut id = String::new();
        let mut entity = EntityPrefab {
            bundles: vec![],
        };

        if let Some(size) = map.size_hint() {
            entity.bundles.reserve(size);
        }

        while let Some(key) = map.next_key::<String>()? {
            if &key == "id" {
                id = map.next_value::<String>()?;
            } else if let Some(implementation) = self.factory.bundles.get(&key) {
                entity.bundles.push(map.next_value_seed(implementation.clone())?);
            } else {
                return Err(A::Error::custom(format!("no such bundle: {key}")));
            }
        }

        Ok(Document {
            id,
            entity,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrefabEntityReference(String);

impl Deref for PrefabEntityReference {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
struct Implementation {
    deserialize: Arc<dyn (Fn(&mut dyn erased_serde::Deserializer<'_>) -> anyhow::Result<Arc<dyn PrefabBundle>>) + Send + Sync + 'static>,
}

impl<'de> DeserializeSeed<'de> for Implementation {
    type Value = Arc<dyn PrefabBundle>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        let mut deserializer = <dyn erased_serde::Deserializer>::erase(deserializer);
        (self.deserialize)(&mut deserializer).map_err(|e| D::Error::custom(e))
    }
}

pub struct Document {
    pub id: String,
    pub entity: EntityPrefab,
}

#[derive(Default, Clone, Resource)]
pub struct PrefabFactory {
    bundles: HashMap<String, Implementation>,
}

impl PrefabFactory {
    pub fn new() -> PrefabFactory {
        Default::default()
    }

    pub fn deserialize_entity<'de, D>(&'de self, deserializer: D) -> Result<Document, <D as Deserializer>::Error> where D: Deserializer<'de> {
        deserializer.deserialize_any(PrefabVisitor { factory: self })
    }

    pub fn deserialize_prefab_yaml(&self, d: serde_yaml::Deserializer) -> anyhow::Result<Prefab> {
        let mut entities = HashMap::new();

        for d in d {
            let prefab = self.deserialize_entity(d)?;
            if entities.contains_key(&prefab.id) {
                return Err(anyhow::anyhow!("duplicate entity ID {}", &prefab.id));
            }
            entities.insert(prefab.id, prefab.entity);
        }

        Ok(Prefab { entities })
    }

    pub fn register(&mut self, name: &str, f: impl Fn(&mut dyn erased_serde::Deserializer<'_>) -> anyhow::Result<Arc<dyn PrefabBundle>> + Sync + Send + 'static) {
        self.bundles.insert(name.into(), Implementation {
            deserialize: Arc::new(f),
        });
    }

    pub fn register_template<P: FromPrefabTemplate>(&mut self, name: &str) {
        self.register(name, |d|
            Ok(Arc::new(P::from_template(P::Template::deserialize(d)?))))
    }
}

#[derive(Default, Clone, Resource)]
pub struct PrefabCollection {
    prefabs: HashMap<String, Arc<Prefab>>,
}

impl PrefabCollection {
    pub fn new() -> PrefabCollection {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.prefabs.len()
    }

    pub fn prefabs(&self) -> &HashMap<String, Arc<Prefab>> {
        &self.prefabs
    }

    pub fn get(&self, id: &str) -> Option<&Arc<Prefab>> {
        self.prefabs.get(id)
    }

    pub async fn load_from_directory(&mut self, factory: &PrefabFactory, path: &Path) -> anyhow::Result<()> {
        let mut to_visit = VecDeque::new();
        to_visit.push_back(path.to_path_buf());

        while let Some(next) = to_visit.pop_front() {
            let mut entries = fs::read_dir(&next).await?;
            while let Some(entry) = entries.next_entry().await? {
                let metadata = entry.metadata().await?;
                if metadata.is_dir() {
                    to_visit.push_back(next.join(entry.file_name()));
                } else if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".yaml") {
                        let full_path = next.join(entry.file_name());
                        let prefab_name = &name[..name.len() - 5];
                        let contents = fs::read_to_string(&full_path).await?;
                        let d = serde_yaml::Deserializer::from_str(&contents);
                        let prefab = factory.deserialize_prefab_yaml(d)
                            .with_context(|| format!("deserializing {:?}", &full_path))?;
                        self.prefabs.insert(prefab_name.into(), Arc::new(prefab));
                    }
                }
            }
        }

        Ok(())
    }
}

pub trait PrefabAppExt {
    fn init_prefab_bundle<P: FromPrefabTemplate>(&mut self, name: &str) -> &mut Self;
}

impl PrefabAppExt for App {
    fn init_prefab_bundle<P: FromPrefabTemplate>(&mut self, name: &str) -> &mut Self {
        self.world.resource_mut::<PrefabFactory>()
            .register_template::<P>(name);
        self
    }
}

pub trait PrefabCommandsExt {
    fn insert_prefab(&mut self, prefab: Arc<Prefab>) -> &mut Self;
}

impl<'w, 's, 'a> PrefabCommandsExt for EntityCommands<'w, 's, 'a> {
    fn insert_prefab(&mut self, prefab: Arc<Prefab>) -> &mut Self {
        let entity = self.id();
        self.commands().add(InsertPrefab { prefab, entity });
        self
    }
}

#[derive(Default)]
pub struct PrefabPlugin;

impl Plugin for PrefabPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<PrefabFactory>()
            .init_resource::<PrefabCollection>()
            .init_prefab_bundle::<InheritancePrefab>("inherit")
            .init_prefab_bundle::<LocationPrefab>("location");
    }
}
