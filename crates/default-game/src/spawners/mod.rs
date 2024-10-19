use std::sync::Arc;
use std::time::Duration;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::prelude::World;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::prelude::*;
use bevy::reflect::{DynamicList, DynamicStruct};
use bevy::time::{Time, Timer, TimerMode};
use bevy::utils::HashMap;
use serde::Deserialize;
use serde_yaml::Value;
use bevy_fabricator::{FabricateExt, Factory};
use bevy_fabricator::loader::Fabricator;
use yewoh_server::world::entity::Location;
use yewoh_server::world::net::NetCommandsExt;

use crate::data::prefab::{FromPrefabTemplate, PrefabAppExt, PrefabBundle};

fn to_reflect(value: &Value) -> anyhow::Result<Box<dyn PartialReflect>> {
    let v = match value {
        Value::Null => Box::new(()) as Box<dyn PartialReflect>,
        Value::Bool(v) => Box::new(*v),
        Value::Number(n) => {
            if let Some(v) = n.as_i64() {
                Box::new(v) as _
            } else if let Some(v) = n.as_u64() {
                Box::new(v) as _
            } else if let Some(v) = n.as_f64() {
                Box::new(v) as _
            } else {
                unreachable!()
            }
        }
        Value::String(s) => Box::new(s.clone()) as _,
        Value::Sequence(seq) => {
            let mut list = DynamicList::default();
            for v in seq.iter() {
                list.push_box(to_reflect(v)?);
            }
            Box::new(list) as _
        }
        Value::Mapping(m) => {
            let mut map = DynamicStruct::default();
            for (k, v) in m.iter() {
                let k = k.as_str().expect("struct keys must be strings");
                map.insert_boxed(k, to_reflect(v)?);
            }
            Box::new(map)
        }
        Value::Tagged(_) => anyhow::bail!("tagged values cannot appear in structs"),
    };
    Ok(v)
}

#[derive(Component)]
struct HookupSpawner {
    prefab: Handle<Fabricator>,
    parameters: Arc<dyn PartialReflect>,
    next_spawn: Timer,
    limit: usize,
}

#[derive(Clone, Default, Reflect, Deserialize)]
pub struct SpawnerPrefab {
    prefab: String,
    #[serde(default)]
    #[reflect(ignore)]
    parameters: HashMap<String, Value>,
    #[serde(with = "humantime_serde")]
    interval: Duration,
    limit: usize,
}

impl PrefabBundle for SpawnerPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        let asset_server = world.resource::<AssetServer>();
        let prefab = asset_server.load(&self.prefab);

        let mut parameters = DynamicStruct::default();

        for (k, v) in &self.parameters {
            parameters.insert_boxed(k, to_reflect(v).unwrap());
        }

        let parameters = Arc::new(parameters) as Arc<dyn PartialReflect>;

        world.entity_mut(entity)
            .insert(HookupSpawner {
                prefab,
                parameters,
                next_spawn: Timer::new(self.interval, TimerMode::Repeating),
                limit: self.limit,
            })
            .insert(SpawnedEntities::default());
    }
}

impl FromPrefabTemplate for SpawnerPrefab {
    type Template = SpawnerPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

fn setup_spawner_prefabs(
    asset_server: Res<AssetServer>,
    prefabs: Res<Assets<Fabricator>>,
    mut commands: Commands, query: Query<(Entity, &HookupSpawner)>,
) {
    for (entity, hookup) in &query {
        debug!("try setup spawner {:?} ({:?})", hookup.prefab, asset_server.get_load_state(&hookup.prefab));
        let Some(prefab) = prefabs.get(&hookup.prefab) else { continue };

        commands
            .entity(entity)
            .remove::<HookupSpawner>()
            .insert(Spawner {
                prefab: prefab.fabricable.fabricate.clone(),
                parameters: hookup.parameters.clone(),
                next_spawn: hookup.next_spawn.clone(),
                limit: hookup.limit,
            });
    }
}

#[derive(Clone, Component)]
pub struct Spawner {
    pub prefab: Factory,
    pub parameters: Arc<dyn PartialReflect>,
    pub next_spawn: Timer,
    pub limit: usize,
}

#[derive(Debug, Clone, Component, Reflect)]
pub struct Spawned;

#[derive(Default, Debug, Clone, Component, Reflect)]
pub struct SpawnedEntities {
    pub entities: Vec<Entity>,
}

pub fn spawn_from_spawners(
    time: Res<Time>,
    mut spawners: Query<(&mut Spawner, &mut SpawnedEntities, &Location)>,
    spawned_entities: Query<(), With<Spawned>>,
    mut commands: Commands,
) {
    for (mut spawner, mut spawned, position) in spawners.iter_mut() {
        spawned.entities.retain(|e| spawned_entities.contains(*e));
        if !spawner.next_spawn.tick(time.delta()).just_finished() || spawner.limit <= spawned.entities.len() {
            continue;
        }

        let spawned_entity = commands.spawn_empty()
            .fabricate(spawner.prefab.clone(), spawner.parameters.clone())
            .insert(Spawned)
            .insert(*position)
            .assign_network_id()
            .id();

        spawned.entities.push(spawned_entity);
    }
}

#[derive(Default)]
pub struct SpawnersPlugin;

impl Plugin for SpawnersPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Update, (
                setup_spawner_prefabs,
                spawn_from_spawners,
            ))
            .init_prefab_bundle::<SpawnerPrefab>("spawner");
    }
}
