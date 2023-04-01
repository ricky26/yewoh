use std::collections::{HashMap, VecDeque};
use std::fmt::Formatter;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use bevy_app::{App, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Commands, EntityCommands, Resource};
use serde::{Deserialize, Deserializer};
use serde::de::{DeserializeSeed, Error, MapAccess, Visitor};
use tokio::fs;

pub trait PrefabBundle: Send + Sync {
    fn spawn(&self, prefab: &Prefab, commands: &mut EntityCommands<'_, '_, '_>);
}

pub trait FromPrefabTemplate: PrefabBundle + 'static {
    type Template: for<'de> Deserialize<'de> + 'static;
    fn from_template(template: Self::Template) -> Self;
}

#[derive(Clone)]
pub struct EntityPrefab {
    bundles: Vec<Arc<dyn PrefabBundle>>,
}

#[derive(Clone)]
pub struct Prefab {
    entities: HashMap<String, EntityPrefab>,
}

impl Prefab {
    pub fn insert_child(&self, id: &str, commands: &mut EntityCommands) {
        if let Some(prefab) = self.entities.get(id) {
            for bundle in &prefab.bundles {
                bundle.spawn(self, commands);
            }
        }
    }

    pub fn spawn_child(&self, id: &str, commands: &mut Commands) -> Option<Entity> {
        if self.entities.contains_key(id) {
            let mut entity = commands.spawn_empty();
            self.insert_child(id, &mut entity);
            Some(entity.id())
        } else {
            None
        }
    }

    pub fn insert(&self, commands: &mut EntityCommands<'_, '_, '_>) {
        self.insert_child(&"", commands);
    }

    pub fn spawn(&self, commands: &mut Commands) -> Entity {
        let mut entity = commands.spawn_empty();
        self.insert(&mut entity);
        entity.id()
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

        while let Some(key) = map.next_key::<&str>()? {
            if key == "id" {
                id = map.next_value::<String>()?;
            } else if let Some(implementation) = self.factory.bundles.get(key) {
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
    deserialize: fn(&mut dyn erased_serde::Deserializer<'_>) -> anyhow::Result<Arc<dyn PrefabBundle>>,
}

impl<'de> DeserializeSeed<'de> for Implementation {
    type Value = Arc<dyn PrefabBundle>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        let mut deserializer = <dyn erased_serde::Deserializer>::erase(deserializer);
        (self.deserialize)(&mut deserializer).map_err(|e| D::Error::custom(e))
    }
}

struct Document {
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

    fn deserialize_impl<P: FromPrefabTemplate>(deserializer: &mut dyn erased_serde::Deserializer<'_>) -> anyhow::Result<Arc<dyn PrefabBundle>> {
        Ok(Arc::new(P::from_template(P::Template::deserialize(deserializer)?)))
    }

    fn deserialize<'de, D>(&'de self, deserializer: D) -> Result<Document, <D as Deserializer>::Error> where D: Deserializer<'de> {
        deserializer.deserialize_map(PrefabVisitor { factory: self })
    }

    pub fn register<P: FromPrefabTemplate>(&mut self, name: &str) {
        self.bundles.insert(name.into(), Implementation {
            deserialize: Self::deserialize_impl::<P>,
        });
    }
}

#[derive(Default, Clone, Resource)]
pub struct PrefabCollection {
    prefabs: HashMap<String, Prefab>,
}

impl PrefabCollection {
    pub fn new() -> PrefabCollection {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.prefabs.len()
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

                        let mut entities = HashMap::new();
                        for d in serde_yaml::Deserializer::from_str(&contents) {
                            let prefab = factory.deserialize(d)
                                .with_context(|| format!("deserializing {:?}", &full_path))?;
                            if entities.contains_key(&prefab.id) {
                                return Err(anyhow::anyhow!("duplicate entity ID {} in {:?}", &prefab.id, &full_path));
                            }
                            entities.insert(prefab.id, prefab.entity);
                        }

                        self.prefabs.insert(prefab_name.into(), Prefab {
                            entities,
                        });
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
            .register::<P>(name);
        self
    }
}

#[derive(Default)]
pub struct PrefabPlugin;

impl Plugin for PrefabPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<PrefabFactory>()
            .init_resource::<PrefabCollection>();
    }
}
