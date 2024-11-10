use bevy::prelude::*;
use yewoh::assets::map::MapTile;
use yewoh::assets::tiles::{TileData, TileFlags};

use crate::world::entity::{Direction, MapPosition};
use crate::world::map::Chunk;
use crate::world::spatial::{Collider, SpatialQuery};

#[derive(Debug, Clone)]
pub enum MoveError {
    Impassable,
    Obstructed(Entity),
}

pub fn try_move_in_direction(
    query: &SpatialQuery,
    chunk_query: &Query<(&MapPosition, &Chunk)>,
    tile_data: &TileData,
    position: MapPosition,
    direction: Direction,
    ignore: Option<Entity>,
) -> Result<MapPosition, MoveError> {
    // Step forward and up 10 units, then drop the character down onto their destination.
    let mut test_position = position.position + direction.as_vec2().extend(10);
    let mut new_z = -1;

    for collider in query.iter_colliders(position.map_id, test_position.truncate()) {
        if Some(collider.entity()) == ignore {
            continue;
        }

        match collider {
            Collider::Chunk(entity) => {
                let Ok((chunk_pos, chunk)) = chunk_query.get(entity) else {
                    continue;
                };

                let chunk_off = test_position.truncate() - chunk_pos.position.truncate();
                let MapTile { tile_id, height } = chunk.map_chunk
                    .get(chunk_off.x as usize, chunk_off.y as usize);

                if !tile_data.land[tile_id as usize].flags.contains(TileFlags::IMPASSABLE) {
                    let z = height as i32;
                    if z <= test_position.z {
                        new_z = new_z.max(z);
                    }
                }
            }
            Collider::StaticItem(entry) | Collider::DynamicItem(entry) => {
                let Some(tile_data) = tile_data.items.get(entry.graphic as usize) else {
                    continue;
                };

                if tile_data.flags.contains(TileFlags::IMPASSABLE) {
                    if test_position.z >= entry.z_min && test_position.z <= entry.z_max {
                        return Err(MoveError::Obstructed(entry.entity));
                    }
                } else if tile_data.flags.contains(TileFlags::SURFACE) && entry.z_max <= test_position.z {
                    new_z = new_z.max(entry.z_max);
                }
            }
        }
    }

    if new_z < 0 {
        Err(MoveError::Impassable)
    } else {
        test_position.z = new_z;
        Ok(MapPosition { map_id: position.map_id, position: test_position, direction })
    }
}
