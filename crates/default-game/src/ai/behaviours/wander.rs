use std::time::Duration;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::prelude::World;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::reflect::Reflect;
use bevy::time::{Time, Timer, TimerMode};
use rand::{thread_rng, Rng};
use serde::Deserialize;

use yewoh::Direction;
use yewoh_server::world::entity::Location;
use yewoh_server::world::map::TileDataResource;
use yewoh_server::world::navigation::try_move_in_direction;
use yewoh_server::world::spatial::EntitySurfaces;

use crate::data::prefab::{FromPrefabTemplate, PrefabBundle};

#[derive(Debug, Clone, Component, Reflect)]
pub struct Wander;

#[derive(Debug, Clone, Component, Reflect)]
pub struct MoveTimer {
    pub next_move: Timer,
}

pub fn wander(
    time: Res<Time>, tile_data: Res<TileDataResource>, surfaces: Res<EntitySurfaces>,
    mut npcs: Query<(Entity, &mut Location, &mut MoveTimer), With<Wander>>,
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

#[derive(Deserialize)]
pub struct WanderPrefab {
    #[serde(with = "humantime_serde")]
    pub interval: Duration,
}

impl FromPrefabTemplate for WanderPrefab {
    type Template = WanderPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for WanderPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        world.entity_mut(entity)
            .insert(Wander)
            .insert(MoveTimer {
                next_move: Timer::new(self.interval, TimerMode::Repeating),
            });
    }
}
