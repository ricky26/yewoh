use std::time::Duration;
use bevy_app::{App, Plugin};

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Commands, EntityCommands, Query, Res};
use bevy_time::{Timer, TimerMode};
use serde_derive::Deserialize;

use crate::data::prefab::{FromPrefabTemplate, Prefab, PrefabAppExt, PrefabBundle, PrefabCollection};
use crate::npc::{SpawnedEntities, Spawner};

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
    fn spawn(&self, _prefab: &Prefab, commands: &mut EntityCommands<'_, '_, '_>) {
        commands
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

#[derive(Default)]
pub struct SpawnersPlugin;

impl Plugin for SpawnersPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_system(setup_spawner_prefabs)
            .init_prefab_bundle::<SpawnerPrefab>("spawner");
    }
}
