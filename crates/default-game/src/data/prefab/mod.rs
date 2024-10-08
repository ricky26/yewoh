use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Formatter};
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use bevy::app::{App, Plugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{EntityCommands, Resource};
use bevy::ecs::world::{Command, EntityWorldMut, World};
use serde::{Deserialize, Deserializer};
use serde::de::{DeserializeSeed, Error, MapAccess, Visitor};
use tokio::fs;

use crate::data::prefab::common::LocationPrefab;
use crate::data::prefab::inheritance::InheritancePrefab;

pub mod inheritance;

pub mod common;

pub trait PrefabBundle: Send + Sync {
    fn write(&self, world: &mut World, entity: Entity);
}

pub trait FromPrefabTemplate: PrefabBundle + 'static {
    type Template: for<'de> Deserialize<'de> + 'static;
    fn from_template(template: Self::Template) -> Self;
}

thread_local! {
    static CURRENT_FACTORY: Cell<*const PrefabFactory> = const { Cell::new(std::ptr::null()) };
}

struct EnterFactoryGuard {
    previous: *const PrefabFactory,
}

impl EnterFactoryGuard {
    pub fn new(next: *const PrefabFactory) -> EnterFactoryGuard {
        EnterFactoryGuard {
            previous: CURRENT_FACTORY.with(|cell| cell.replace(next)),
        }
    }
}

impl Drop for EnterFactoryGuard {
    fn drop(&mut self) {
        CURRENT_FACTORY.with(|ptr| ptr.set(self.previous));
    }
}

#[derive(Clone, Default)]
pub struct Prefab {
    bundles: Vec<Arc<dyn PrefabBundle>>,
}

impl Debug for Prefab {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Prefab {{ {} bundles }}", self.bundles.len())
    }
}

impl Prefab {
    pub fn write(&self, world: &mut World, entity: Entity) {
        for bundle in &self.bundles {
            bundle.write(world, entity);
        }
    }
}

impl<'de> Deserialize<'de> for Prefab {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let factory_ptr = CURRENT_FACTORY.with(|cell| cell.get());
        if factory_ptr.is_null() {
            panic!("tried to deserialize Prefab outside of factory");
        }

        deserializer.deserialize_map(PrefabVisitor {
            factory: unsafe { &*factory_ptr },
        })
    }
}

pub struct InsertPrefab {
    pub entity: Entity,
    pub prefab: Arc<Prefab>,
}

impl Command for InsertPrefab {
    fn apply(self, world: &mut World) {
        self.prefab.write(world, self.entity);
    }
}

struct PrefabVisitor<'a> {
    factory: &'a PrefabFactory,
}

impl<'de, 'a> Visitor<'de> for PrefabVisitor<'a> {
    type Value = Prefab;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "prefab struct")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        let mut prefab = Prefab {
            bundles: vec![],
        };

        if let Some(size) = map.size_hint() {
            prefab.bundles.reserve(size);
        }

        while let Some(key) = map.next_key::<String>()? {
            if let Some(implementation) = self.factory.bundles.get(&key) {
                prefab.bundles.push(map.next_value_seed(implementation.clone())?);
            } else {
                return Err(A::Error::custom(format!("no such bundle: {key}")));
            }
        }

        Ok(prefab)
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
        (self.deserialize)(&mut deserializer).map_err(D::Error::custom)
    }
}

#[derive(Default, Clone, Resource)]
pub struct PrefabFactory {
    bundles: HashMap<String, Implementation>,
}

impl PrefabFactory {
    pub fn new() -> PrefabFactory {
        Default::default()
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

    pub fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        let _guard = EnterFactoryGuard::new(self as *const _);

        f()
    }
}

#[derive(Default, Clone, Resource)]
pub struct PrefabCollection {
    prefabs: HashMap<Arc<str>, Arc<Prefab>>,
}

impl PrefabCollection {
    pub fn new() -> PrefabCollection {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.prefabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn prefabs(&self) -> &HashMap<Arc<str>, Arc<Prefab>> {
        &self.prefabs
    }

    pub fn get(&self, id: &str) -> Option<&Arc<Prefab>> {
        self.prefabs.get(id)
    }

    pub fn get_key_value(&self, id: &str) -> Option<(&Arc<str>, &Arc<Prefab>)> {
        self.prefabs.get_key_value(id)
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
                        let prefab = factory.with(|| Prefab::deserialize(d))
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
        self.world_mut().resource_mut::<PrefabFactory>()
            .register_template::<P>(name);
        self
    }
}

pub trait PrefabCommandsExt {
    fn insert_prefab(&mut self, prefab: Arc<Prefab>) -> &mut Self;
}

impl<'w> PrefabCommandsExt for EntityWorldMut<'w> {
    fn insert_prefab(&mut self, prefab: Arc<Prefab>) -> &mut Self {
        let command = InsertPrefab { prefab, entity: self.id() };
        self.world_scope(|world| command.apply(world));
        self
    }
}

impl<'w> PrefabCommandsExt for EntityCommands<'w> {
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
