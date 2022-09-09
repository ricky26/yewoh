use std::collections::HashMap;

use bevy_ecs::prelude::*;
use quadtree_rs::{point::Point, Quadtree};

use yewoh_server::world::entity::MapPosition;

#[derive(Default)]
pub struct Space {
    quadtrees: HashMap<u8, Quadtree<u32, Entity>>,
    entities: HashMap<Entity, (u8, u64)>,
}

impl Space {
    pub fn quadtree_for_map(&mut self, map_id: u8) -> &mut Quadtree<u32, Entity> {
        self.quadtrees
            .entry(map_id)
            .or_insert_with(|| Quadtree::new(10))
    }
}

pub fn update_space(
    mut space: ResMut<Space>,
    query: Query<
        (Entity, &MapPosition),
        Changed<MapPosition>,
    >,
    removals: RemovedComponents<MapPosition>,
) {
    for (entity, position) in query.iter() {
        let tree = space.quadtree_for_map(position.map_id);
        let handle = tree.insert_pt(Point {
            x: position.position.x,
            y: position.position.y,
        }, entity).unwrap();
        space.entities.insert(entity, (position.map_id, handle));
    }

    for entity in removals.iter() {
        let (map_id, handle) = match space.entities.remove(&entity) {
            Some(x) => x,
            None => continue,
        };

        let tree = match space.quadtrees.get_mut(&map_id) {
            Some(x) => x,
            None => continue,
        };

        tree.delete_by_handle(handle);
    }
}
