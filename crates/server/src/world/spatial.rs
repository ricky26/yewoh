use std::collections::HashMap;
use std::ops::Sub;

use bevy_ecs::prelude::*;
use glam::{IVec2, IVec3, Vec3};
use rstar::{AABB, Point, PointDistance, RTree, RTreeObject};
use rstar::primitives::Rectangle;
use yewoh::assets::map::CHUNK_SIZE;

use crate::world::entity::MapPosition;
use crate::world::map::{Chunk, Surface};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialPoint(pub Vec3);

impl Point for SpatialPoint {
    type Scalar = f32;

    const DIMENSIONS: usize = 3;

    fn generate(mut generator: impl FnMut(usize) -> Self::Scalar) -> Self {
        let x = generator(0);
        let y = generator(1);
        let z = generator(2);
        SpatialPoint(Vec3::new(x, y, z))
    }

    fn nth(&self, index: usize) -> Self::Scalar {
        match index {
            0 => self.0.x,
            1 => self.0.y,
            2 => self.0.z,
            _ => unreachable!(),
        }
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        match index {
            0 => &mut self.0.x,
            1 => &mut self.0.y,
            2 => &mut self.0.z,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    entity: Entity,
    aabb: Rectangle<SpatialPoint>,
}

impl RTreeObject for Entry {
    type Envelope = AABB<SpatialPoint>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb.envelope()
    }
}

impl PointDistance for Entry {
    fn distance_2(&self, point: &SpatialPoint) -> f32 {
        let delta = self.aabb.nearest_point(point).0.sub(point.0);
        delta.x * delta.x + delta.y * delta.y + delta.z * delta.z
    }

    fn contains_point(&self, point: &SpatialPoint) -> bool {
        self.aabb.contains_point(point)
    }

    fn distance_2_if_less_or_equal(&self, point: &SpatialPoint, max_distance_2: f32) -> Option<f32> {
        let distance_2 = self.distance_2(point);
        if distance_2 <= max_distance_2 {
            Some(distance_2)
        } else {
            None
        }
    }
}

struct MaybeIter<T>(Option<T>);

impl<T: Iterator> Iterator for MaybeIter<T> {
    type Item = T::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut iter) = &mut self.0 {
            iter.next()
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpatialEntityTree {
    trees: HashMap<u8, RTree<Entry>>,
    entities: HashMap<Entity, (u8, Rectangle<SpatialPoint>)>,
}

impl SpatialEntityTree {
    fn ensure_tree(&mut self, map: u8) -> &mut RTree<Entry> {
        self.trees.entry(map)
            .or_insert_with(|| RTree::new())
    }

    fn insert(&mut self, entity: Entity, map: u8, aabb: Rectangle<SpatialPoint>) {
        match self.entities.get(&entity) {
            Some((old_map, old_aabb)) if (*old_map == map) && (old_aabb == &aabb) => return,
            Some(_) => self.remove(entity),
            _ => {}
        }

        self.ensure_tree(map).insert(Entry { entity, aabb });
        self.entities.insert(entity, (map, aabb));
    }

    pub fn insert_aabb(&mut self, entity: Entity, map: u8, min: IVec3, max: IVec3) {
        let aabb = Rectangle::from_corners(SpatialPoint(min.as_vec3()), SpatialPoint(max.as_vec3()));
        self.insert(entity, map, aabb);
    }

    pub fn insert_point(&mut self, entity: Entity, map: u8, position: IVec3) {
        let aabb = AABB::from_point(SpatialPoint(position.as_vec3())).into();
        self.insert(entity, map, aabb);
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some((map, aabb)) = self.entities.remove(&entity) {
            if let Some(tree) = self.trees.get_mut(&map) {
                let entry = Entry { entity, aabb };
                tree.remove(&entry);
            }
        }
    }

    pub fn iter(&self, map_id: u8) -> impl Iterator<Item=(Entity, IVec3, IVec3)> + '_ {
        self.trees.get(&map_id).unwrap()
            .iter()
            .map(|e| (e.entity, e.aabb.lower().0.as_ivec3(), e.aabb.upper().0.as_ivec3()))
    }

    pub fn iter_at_point(&self, map: u8, position: IVec3) -> impl Iterator<Item=(Entity, IVec3, IVec3)> + '_ {
        MaybeIter(if let Some(tree) = self.trees.get(&map) {
            Some(tree.locate_all_at_point(&SpatialPoint(position.as_vec3()))
                .map(|e| (e.entity, e.aabb.lower().0.as_ivec3(), e.aabb.upper().0.as_ivec3())))
        } else {
            None
        })
    }

    pub fn iter_at_column(&self, map: u8, position: IVec2) -> impl Iterator<Item=(Entity, IVec3, IVec3)> + '_ {
        let min = SpatialPoint(position.extend(-1000).as_vec3());
        let max = SpatialPoint(position.extend(1000).as_vec3());
        let aabb = AABB::from_corners(min, max);
        MaybeIter(if let Some(tree) = self.trees.get(&map) {
            Some(tree.locate_in_envelope_intersecting(&aabb)
                .map(|e| (e.entity, e.aabb.lower().0.as_ivec3(), e.aabb.upper().0.as_ivec3())))
        } else {
            None
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct EntitySurfaces {
    pub tree: SpatialEntityTree,
}

pub fn update_entity_surfaces(
    mut storage: ResMut<EntitySurfaces>,
    chunks: Query<(Entity, &MapPosition), (With<Chunk>, Or<(Changed<MapPosition>, Changed<Chunk>)>)>,
    surfaces: Query<(Entity, &MapPosition, &Surface), Or<(Changed<MapPosition>, Changed<Surface>)>>,
    removed_chunks: RemovedComponents<Chunk>,
    removed_surfaces: RemovedComponents<Surface>,
) {
    for (entity, position) in chunks.iter() {
        let min = position.position - IVec3::new(0, 0, 1000);
        let max = position.position + IVec3::new(CHUNK_SIZE as i32 - 1, CHUNK_SIZE as i32 - 1, 1000);
        storage.tree.insert_aabb(entity, position.map_id, min, max);
    }

    for (entity, position, surface) in surfaces.iter() {
        storage.tree.insert_point(entity, position.map_id,
            position.position + IVec3::new(0, 0, surface.offset as i32));
    }

    for entity in removed_chunks.iter() {
        storage.tree.remove(entity);
    }

    for entity in removed_surfaces.iter() {
        storage.tree.remove(entity);
    }
}
