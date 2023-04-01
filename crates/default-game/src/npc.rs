use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use bevy_time::{Time, Timer, TimerMode};
use rand::{Rng, thread_rng};

use yewoh::Direction;
use yewoh_server::world::entity::MapPosition;
use yewoh_server::world::map::TileDataResource;
use yewoh_server::world::navigation::try_move_in_direction;
use yewoh_server::world::net::NetCommandsExt;
use yewoh_server::world::spatial::EntitySurfaces;

use crate::data::prefab::Prefab;

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

#[derive(Debug, Clone, Component, Reflect)]
pub struct Npc;

#[derive(Debug, Clone, Component, Reflect)]
pub struct MoveTimer {
    pub next_move: Timer,
}

pub fn spawn_npcs(
    time: Res<Time>,
    mut spawners: Query<(&mut Spawner, &mut SpawnedEntities, &MapPosition)>,
    spawned_entities: Query<With<Spawned>>,
    mut commands: Commands,
) {
    for (mut spawner, mut spawned, position) in spawners.iter_mut() {
        spawned.entities.retain(|e| spawned_entities.contains(*e));
        if !spawner.next_spawn.tick(time.delta()).just_finished() || spawner.limit <= spawned.entities.len() {
            continue;
        }

        let spawned_entity = spawner.prefab.spawn(&mut commands)
            .insert(Spawned)
            .insert(Npc)
            .insert(*position)
            .insert(MoveTimer {
                next_move: Timer::new(Duration::from_secs(1), TimerMode::Repeating),
            })
            .assign_network_id()
            .id();
        spawned.entities.push(spawned_entity);
    }
}

pub fn move_npcs(
    time: Res<Time>, tile_data: Res<TileDataResource>, surfaces: Res<EntitySurfaces>,
    mut npcs: Query<(Entity, &mut MapPosition, &mut MoveTimer), With<Npc>>,
) {
    let mut rng = thread_rng();

    for (entity, mut position, mut move_timer) in npcs.iter_mut() {
        if !move_timer.next_move.tick(time.delta()).just_finished() {
            continue;
        }

        let direction = Direction::from_repr(rng.gen_range(0..8)).unwrap();
        if let Ok(new_position) = try_move_in_direction(&surfaces, &tile_data, *position, direction, Some(entity)) {
            *position = new_position;
        }
    }
}
