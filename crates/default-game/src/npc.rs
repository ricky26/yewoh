use std::time::Duration;

use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use bevy_time::{Time, Timer, TimerMode};
use glam::IVec3;
use rand::{Rng, thread_rng};

use yewoh::{Direction, Notoriety};
use yewoh_server::world::entity::{Character, Flags, MapPosition, Notorious, Stats};
use yewoh_server::world::map::TileDataResource;
use yewoh_server::world::navigation::try_move_in_direction;
use yewoh_server::world::net::{NetEntity, NetEntityAllocator};
use yewoh_server::world::spatial::EntitySurfaces;
use crate::characters::Alive;

#[derive(Debug, Clone, Component, Reflect)]
pub struct Spawner {
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

pub fn init_npcs(mut commands: Commands) {
    commands.spawn((
        MapPosition {
            map_id: 1,
            position: IVec3::new(1325, 1624, 55),
            direction: Direction::North,
        },
        Spawner {
            next_spawn: Timer::new(Duration::from_secs(5), TimerMode::Repeating),
            limit: 5,
        },
        SpawnedEntities::default(),
    ));
}

pub fn spawn_npcs(
    time: Res<Time>,
    allocator: Res<NetEntityAllocator>,
    mut spawners: Query<(&mut Spawner, &mut SpawnedEntities, &MapPosition)>,
    spawned_entities: Query<With<Spawned>>,
    mut commands: Commands,
) {
    for (mut spawner, mut spawned, position) in spawners.iter_mut() {
        spawned.entities.retain(|e| spawned_entities.contains(*e));
        if !spawner.next_spawn.tick(time.delta()).just_finished() || spawner.limit <= spawned.entities.len() {
            continue;
        }

        let spawned_entity = commands.spawn((
            NetEntity { id: allocator.allocate_character() },
            Flags::default(),
            *position,
            Notorious(Notoriety::Enemy),
            Character {
                body_type: 0xee,
                hue: 0x43,
                equipment: vec![],
            },
            Stats {
                name: "Mr Rat".into(),
                hp: 13,
                max_hp: 112,
                ..Default::default()
            },
            Npc,
            Spawned,
            Alive,
            MoveTimer {
                next_move: Timer::new(Duration::from_secs(1), TimerMode::Repeating),
            },
        )).id();
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
