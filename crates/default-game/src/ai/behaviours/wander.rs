use std::time::Duration;

use bevy::prelude::*;
use rand::{thread_rng, Rng};
use serde::Deserialize;
use bevy_fabricator::Fabricated;
use bevy_fabricator::traits::{Apply, ReflectApply};
use yewoh::Direction;
use yewoh_server::world::entity::MapPosition;
use yewoh_server::world::map::{Chunk, TileDataResource};
use yewoh_server::world::navigation::try_move_in_direction;
use yewoh_server::world::spatial::SpatialQuery;

#[derive(Debug, Clone, Component, Reflect)]
pub struct Wander;

#[derive(Debug, Clone, Component, Reflect)]
pub struct MoveTimer {
    pub next_move: Timer,
}

pub fn wander(
    time: Res<Time>,
    tile_data: Res<TileDataResource>,
    spatial_query: SpatialQuery,
    chunk_query: Query<(&MapPosition, &Chunk)>,
    mut npcs: Query<(Entity, &mut MapPosition, &mut MoveTimer), (Without<Chunk>, With<Wander>)>,
) {
    let mut rng = thread_rng();

    for (entity, mut position, mut move_timer) in npcs.iter_mut() {
        if !move_timer.next_move.tick(time.delta()).just_finished() {
            continue;
        }

        let direction = Direction::from_repr(rng.gen_range(0..8)).unwrap();
        if let Ok(new_position) = try_move_in_direction(&spatial_query, &chunk_query, &tile_data, *position, direction, Some(entity)) {
            *position = new_position;
        }
    }
}

#[derive(Clone, Default, Reflect, Deserialize)]
#[reflect(Default, Apply, Deserialize)]
pub struct WanderPrefab {
    #[serde(with = "humantime_serde")]
    pub interval: Duration,
}

impl Apply for WanderPrefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        world.entity_mut(entity)
            .insert(Wander)
            .insert(MoveTimer {
                next_move: Timer::new(self.interval, TimerMode::Repeating),
            });
        Ok(())
    }
}
