use std::sync::Arc;
use std::time::Duration;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::prelude::*;
use bevy::reflect::{DynamicList, DynamicStruct};
use bevy::time::{Time, Timer, TimerMode};
use bevy::utils::HashMap;
use bevy_fabricator::traits::{Apply, Context, ReflectApply};
use serde::Deserialize;
use serde_yaml::Value;
use yewoh_server::world::entity::MapPosition;

use crate::data::prefabs::{PrefabLibraryEntityExt, PrefabLibraryRequest};

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

#[derive(Clone, Default, Reflect, Deserialize)]
#[reflect(Apply, Deserialize)]
pub struct SpawnerPrefab {
    prefab: String,
    #[serde(default)]
    #[reflect(ignore)]
    parameters: HashMap<String, Value>,
    #[serde(with = "humantime_serde")]
    interval: Duration,
    limit: usize,
}

impl Apply for SpawnerPrefab {
    fn apply(&self, ctx: &mut Context, entity: Entity) -> anyhow::Result<()> {
        let mut parameters = DynamicStruct::default();

        for (k, v) in &self.parameters {
            parameters.insert_boxed(k, to_reflect(v).unwrap());
        }

        let parameters = Arc::new(parameters) as Arc<dyn PartialReflect>;
        ctx.world.entity_mut(entity)
            .insert(Spawner {
                prefab: self.prefab.clone(),
                parameters,
                next_spawn: Timer::new(self.interval, TimerMode::Repeating),
                limit: self.limit,
            })
            .insert(SpawnedEntities::default());
        Ok(())
    }
}

#[derive(Clone, Component)]
pub struct Spawner {
    pub prefab: String,
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
    mut spawners: Query<(&mut Spawner, &mut SpawnedEntities, &MapPosition)>,
    spawned_entities: Query<(), With<Spawned>>,
    mut commands: Commands,
) {
    for (mut spawner, mut spawned, position) in spawners.iter_mut() {
        spawned.entities.retain(|e| spawned_entities.contains(*e));
        if !spawner.next_spawn.tick(time.delta()).just_finished() || spawner.limit <= spawned.entities.len() {
            continue;
        }

        let request = PrefabLibraryRequest {
            prefab_name: spawner.prefab.clone(),
            parameters: spawner.parameters.clone(),
        };
        let spawned_entity = commands.spawn_empty()
            .fabricate_from_library(request)
            .insert((
                Spawned,
                *position,
            ))
            .insert(*position)
            .id();

        spawned.entities.push(spawned_entity);
    }
}

#[derive(Default)]
pub struct SpawnersPlugin;

impl Plugin for SpawnersPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<SpawnerPrefab>()
            .register_type::<Spawned>()
            .register_type::<SpawnedEntities>()
            .add_systems(Update, (
                spawn_from_spawners,
            ));
    }
}
