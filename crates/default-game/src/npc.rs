use std::time::Duration;
use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use bevy_time::{Time, Timer, TimerMode};
use glam::IVec3;

use yewoh::{Direction, Notoriety};
use yewoh_server::world::entity::{Character, Flags, MapPosition, Notorious, Stats};
use yewoh_server::world::net::{NetEntity, NetEntityAllocator};

#[derive(Debug, Clone, Component, Reflect)]
pub struct Spawner {
    pub next_spawn: Timer,
}

pub fn init_npcs(mut commands: Commands) {
    commands.spawn((
        MapPosition {
            map_id: 0,
            position: IVec3::new(1325, 1624, 55),
            direction: Direction::North,
        },
        Spawner {
            next_spawn: Timer::new(Duration::from_secs(5), TimerMode::Repeating),
        },
    ));
}

pub fn spawn_npcs(
    time: Res<Time>,
    allocator: Res<NetEntityAllocator>,
    mut spawners: Query<(&mut Spawner, &MapPosition)>,
    mut commands: Commands,
) {
    for (mut spawner, position) in spawners.iter_mut() {
        if !spawner.next_spawn.tick(time.delta()).finished() {
            continue;
        }

        commands.spawn((
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
            }));
    }
}
