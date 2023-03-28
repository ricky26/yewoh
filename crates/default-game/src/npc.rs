use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use glam::IVec3;

use yewoh::{Direction, Notoriety};
use yewoh_server::world::entity::{Character, Flags, MapPosition, Notorious, Stats};
use yewoh_server::world::net::{NetEntity, NetEntityAllocator};
use yewoh_server::world::time::Tick;

#[derive(Debug, Clone, Component, Reflect)]
pub struct Spawner {
    pub next_spawn: u32,
}

pub fn init_npcs(tick: Res<Tick>, mut commands: Commands) {
    commands.spawn((
        MapPosition {
            map_id: 0,
            position: IVec3::new(1325, 1624, 55),
            direction: Direction::North,
        },
        Spawner { next_spawn: tick.tick + 10 },
    ));
}

pub fn spawn_npcs(
    tick: Res<Tick>,
    allocator: Res<NetEntityAllocator>,
    spawners: Query<(&mut Spawner, &MapPosition)>,
    mut commands: Commands,
) {
    for (spawner, position) in spawners.iter() {
        if spawner.next_spawn != tick.tick {
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
