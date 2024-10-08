use std::sync::Arc;
use std::time::Duration;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::prelude::World;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::prelude::*;
use bevy::time::{Time, Timer, TimerMode};
use serde::Deserialize;
use yewoh_server::world::entity::Location;
use yewoh_server::world::net::NetCommandsExt;

use crate::data::prefab::PrefabCommandsExt;
use crate::data::prefab::{FromPrefabTemplate, Prefab, PrefabAppExt, PrefabBundle, PrefabCollection};

#[derive(Component)]
struct HookupSpawner {
    prefab: String,
    next_spawn: Timer,
    limit: usize,
}

#[derive(Deserialize)]
pub struct SpawnerPrefab {
    prefab: String,
    #[serde(with = "humantime_serde")]
    interval: Duration,
    limit: usize,
}

impl PrefabBundle for SpawnerPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        world.entity_mut(entity)
            .insert(HookupSpawner {
                prefab: self.prefab.clone(),
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

fn setup_spawner_prefabs(prefabs: Res<PrefabCollection>, mut commands: Commands, query: Query<(Entity, &HookupSpawner)>) {
    for (entity, hookup) in &query {
        let prefab = match prefabs.get(&hookup.prefab) {
            Some(x) => x.clone(),
            _ => continue,
        };

        commands
            .entity(entity)
            .remove::<HookupSpawner>()
            .insert(Spawner {
                prefab,
                next_spawn: hookup.next_spawn.clone(),
                limit: hookup.limit,
            });
    }
}

#[derive(Debug, Clone, Component)]
pub struct Spawner {
    pub prefab: Arc<Prefab>,
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
            .insert_prefab(spawner.prefab.clone())
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
