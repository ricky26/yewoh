use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Sub;

use bevy_ecs::prelude::*;
use glam::{IVec2, IVec3};
use rstar::{AABB, Envelope, Point, PointDistance, RTree, RTreeObject, SelectionFunction};
use rstar::primitives::Rectangle;

use yewoh::assets::map::CHUNK_SIZE;

use crate::world::entity::MapPosition;
use crate::world::map::{Chunk, Surface};
use crate::world::net::{NetClient, NetOwner, Possessing, View};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialPoint(pub IVec2);

impl Point for SpatialPoint {
    type Scalar = i32;

    const DIMENSIONS: usize = 2;

    fn generate(mut generator: impl FnMut(usize) -> Self::Scalar) -> Self {
        let x = generator(0);
        let y = generator(1);
        SpatialPoint(IVec2::new(x, y))
    }

    fn nth(&self, index: usize) -> Self::Scalar {
        match index {
            0 => self.0.x,
            1 => self.0.y,
            _ => unreachable!(),
        }
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        match index {
            0 => &mut self.0.x,
            1 => &mut self.0.y,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry<T = ()> {
    entity: Entity,
    metadata: T,
    aabb: Rectangle<SpatialPoint>,
}

impl<T> RTreeObject for Entry<T> {
    type Envelope = AABB<SpatialPoint>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb.envelope()
    }
}

impl<T> PointDistance for Entry<T> {
    fn distance_2(&self, point: &SpatialPoint) -> i32 {
        let delta = self.aabb.nearest_point(point).0.sub(point.0);
        delta.x * delta.x + delta.y * delta.y
    }

    fn contains_point(&self, point: &SpatialPoint) -> bool {
        self.aabb.contains_point(point)
    }

    fn distance_2_if_less_or_equal(&self, point: &SpatialPoint, max_distance_2: i32) -> Option<i32> {
        let distance_2 = self.distance_2(point);
        if distance_2 <= max_distance_2 {
            Some(distance_2)
        } else {
            None
        }
    }
}

struct SelectEntityFunction<T>
{
    entity: Entity,
    aabb: AABB<SpatialPoint>,
    _metadata: PhantomData<T>,
}

impl<T> SelectionFunction<Entry<T>> for SelectEntityFunction<T>
{
    fn should_unpack_parent(&self, parent_envelope: &AABB<SpatialPoint>) -> bool {
        parent_envelope.contains_envelope(&self.aabb)
    }

    fn should_unpack_leaf(&self, leaf: &Entry<T>) -> bool {
        leaf.entity == self.entity
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
pub struct SpatialEntityTree<T = ()> {
    trees: HashMap<u8, RTree<Entry<T>>>,
    entities: HashMap<Entity, (u8, Rectangle<SpatialPoint>)>,
}

impl<T> SpatialEntityTree<T> {
    fn ensure_tree(&mut self, map: u8) -> &mut RTree<Entry<T>> {
        self.trees.entry(map)
            .or_insert_with(|| RTree::new())
    }

    fn insert(&mut self, entity: Entity, metadata: T, map: u8, aabb: Rectangle<SpatialPoint>) {
        match self.entities.get(&entity) {
            Some((old_map, old_aabb)) if (*old_map == map) && (old_aabb == &aabb) => return,
            Some(_) => self.remove(entity),
            _ => {}
        }

        self.ensure_tree(map).insert(Entry { entity, metadata, aabb });
        self.entities.insert(entity, (map, aabb));
    }

    pub fn insert_aabb(&mut self, entity: Entity, metadata: T, map: u8, min: IVec2, max: IVec2) {
        let aabb = Rectangle::from_corners(SpatialPoint(min), SpatialPoint(max));
        self.insert(entity, metadata, map, aabb);
    }

    pub fn insert_point(&mut self, entity: Entity, metadata: T, map: u8, position: IVec2) {
        let aabb = AABB::from_point(SpatialPoint(position)).into();
        self.insert(entity, metadata, map, aabb);
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some((map, aabb)) = self.entities.remove(&entity) {
            if let Some(tree) = self.trees.get_mut(&map) {
                tree.remove_with_selection_function(SelectEntityFunction {
                    entity,
                    aabb: aabb.envelope(),
                    _metadata: Default::default(),
                });
            }
        }
    }

    pub fn iter(&self, map_id: u8) -> impl Iterator<Item=(Entity, IVec2, IVec2)> + '_ {
        self.trees.get(&map_id).unwrap()
            .iter()
            .map(|e| (e.entity, e.aabb.lower().0, e.aabb.upper().0))
    }

    pub fn iter_aabb(&self, map_id: u8, min: IVec2, max: IVec2) -> impl Iterator<Item=Entity> + '_ {
        MaybeIter(if let Some(tree) = self.trees.get(&map_id) {
            let envelope = AABB::from_corners(SpatialPoint(min), SpatialPoint(max));
            Some(tree.locate_in_envelope_intersecting(&envelope)
                .map(|e| e.entity))
        } else {
            None
        })
    }

    pub fn iter_at_point(&self, map: u8, position: IVec2) -> impl Iterator<Item=Entity> + '_ {
        MaybeIter(if let Some(tree) = self.trees.get(&map) {
            Some(tree.locate_all_at_point(&SpatialPoint(position))
                .map(|e| e.entity))
        } else {
            None
        })
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct EntitySurfaces {
    pub tree: SpatialEntityTree<u8>,
}

pub fn update_entity_surfaces(
    mut storage: ResMut<EntitySurfaces>,
    chunks: Query<(Entity, &MapPosition), (With<Chunk>, Or<(Changed<MapPosition>, Changed<Chunk>)>)>,
    surfaces: Query<(Entity, &MapPosition, &Surface), Or<(Changed<MapPosition>, Changed<Surface>)>>,
    mut removed_chunks: RemovedComponents<Chunk>,
    mut removed_surfaces: RemovedComponents<Surface>,
) {
    for (entity, position) in chunks.iter() {
        let min = position.position.truncate();
        let max = min + IVec2::new(CHUNK_SIZE as i32 - 1, CHUNK_SIZE as i32 - 1);
        storage.tree.insert_aabb(entity, 0, position.map_id, min, max);
    }

    for (entity, position, surface) in surfaces.iter() {
        storage.tree.insert_point(entity, surface.offset, position.map_id, position.position.truncate());
    }

    for entity in removed_chunks.iter() {
        storage.tree.remove(entity);
    }

    for entity in removed_surfaces.iter() {
        storage.tree.remove(entity);
    }
}

#[derive(Debug, Clone, Default, Component)]
pub struct Size {
    min: IVec3,
    max: IVec3,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct EntityPositions {
    pub tree: SpatialEntityTree,
}

pub fn update_entity_positions(
    mut storage: ResMut<EntityPositions>,
    entities: Query<(Entity, &MapPosition, Option<&Size>), Or<(Changed<MapPosition>, Changed<Size>)>>,
    mut removed_entities: RemovedComponents<MapPosition>,
) {
    for (entity, position, size) in &entities {
        let size = size.cloned().unwrap_or(Size::default());
        let min = position.position - size.min;
        let max = position.position + size.max;
        storage.tree.insert_aabb(entity, (), position.map_id, min.truncate(), max.truncate());
    }

    for entity in removed_entities.iter() {
        storage.tree.remove(entity);
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct NetClientPositions {
    pub tree: SpatialEntityTree,
}

pub fn update_client_positions(
    mut storage: ResMut<NetClientPositions>,
    clients: Query<(Entity, &View, &Possessing), With<NetClient>>,
    characters: Query<&MapPosition>,
    changed_clients: Query<
        (Entity, &View, &Possessing),
        (With<NetClient>, Or<(Changed<View>, Changed<Possessing>)>),
    >,
    changed_characters: Query<(Entity, &NetOwner, &MapPosition), Changed<MapPosition>>,
    mut removed_clients: RemovedComponents<NetClient>,
) {
    for (entity, view, possessing) in changed_clients.iter() {
        let map_position = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        let size = IVec2::splat(view.range as i32);
        let position = map_position.position.truncate();
        let min = position - size;
        let max = position + size;
        storage.tree.insert_aabb(entity, (), map_position.map_id, min, max);
    }

    for (possessed_entity, owner, map_position) in changed_characters.iter() {
        let (entity, view, possessing) = match clients.get(owner.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        if possessing.entity != possessed_entity {
            continue;
        }

        let size = IVec2::splat(view.range as i32);
        let position = map_position.position.truncate();
        let min = position - size;
        let max = position + size;
        storage.tree.insert_aabb(entity, (), map_position.map_id, min, max);
    }

    for entity in removed_clients.iter() {
        storage.tree.remove(entity);
    }
}
