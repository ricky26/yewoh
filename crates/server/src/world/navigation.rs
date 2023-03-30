use bevy_ecs::entity::Entity;
use yewoh::assets::tiles::{TileData, TileFlags};

use yewoh::Direction;

use crate::world::entity::MapPosition;
use crate::world::spatial::{EntitySurfaces, SurfaceKind};

#[derive(Debug, Clone)]
pub enum MoveError {
    Impassable,
    Obstructed(Entity),
}

pub fn try_move_in_direction(surfaces: &EntitySurfaces, tile_data: &TileData, position: MapPosition, direction: Direction, ignore: Option<Entity>) -> Result<MapPosition, MoveError> {
    // Step forward and up 10 units, then drop the character down onto their destination.
    let mut test_position = position.position + direction.as_vec2().extend(10);
    let mut new_z = -1;

    for (entity, kind) in surfaces.tree.iter_at_point(position.map_id, test_position.truncate()) {
        if Some(entity) == ignore {
            continue;
        }

        match kind {
            SurfaceKind::Chunk { position, chunk } => {
                let chunk_pos = test_position.truncate() - *position;
                let (tile_id, z) = chunk.get(chunk_pos.x as usize, chunk_pos.y as usize);

                if !tile_data.land[tile_id as usize].flags.contains(TileFlags::IMPASSABLE) {
                    let z = z as i32;
                    if z <= test_position.z {
                        new_z = new_z.max(z);
                    }
                }
            }
            SurfaceKind::Item { min_z, max_z, impassable, .. } => {
                if *impassable {
                    if test_position.z >= *min_z && test_position.z <= *max_z {
                        return Err(MoveError::Obstructed(entity));
                    }
                } else if *max_z <= test_position.z {
                    new_z = new_z.max(*max_z);
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
