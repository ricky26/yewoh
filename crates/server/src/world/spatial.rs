use std::collections::HashMap;
use std::marker::PhantomData;

use bevy_ecs::prelude::*;
use glam::{IVec2, IVec3};
use rstar::{AABB, Envelope, Point, PointDistance, RTree, RTreeObject, SelectionFunction};

use yewoh::assets::map::{CHUNK_SIZE, MapChunk};

use crate::world::entity::{Graphic, Location};
use crate::world::map::{Chunk, Impassable, Surface, TileDataResource};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoundingBox {
    min: IVec2,
    max: IVec2,
}

impl BoundingBox {
    pub fn is_empty(&self) -> bool {
        self.min.x >= self.max.x && self.min.y >= self.max.y
    }

    pub fn aabb(&self) -> AABB<SpatialPoint> {
        AABB::from_corners(SpatialPoint(self.min), SpatialPoint(self.max))
    }

    pub fn empty() -> Self {
        Default::default()
    }

    pub fn from_point(p: IVec2) -> Self {
        Self {
            min: p,
            max: p + IVec2::ONE,
        }
    }

    pub fn from_bounds(min: IVec2, max: IVec2) -> Self {
        Self {
            min: min.min(max),
            max: max.max(min),
        }
    }
}

impl Envelope for BoundingBox {
    type Point = SpatialPoint;

    fn new_empty() -> Self {
        Default::default()
    }

    fn contains_point(&self, point: &Self::Point) -> bool {
        let p = point.0;
        p.x >= self.min.x &&
            p.y >= self.min.y &&
            p.x < self.max.x &&
            p.y < self.max.y
    }

    fn contains_envelope(&self, aabb: &Self) -> bool {
        aabb.min.x >= self.min.x &&
            aabb.min.y >= self.min.y &&
            aabb.max.x <= self.max.x &&
            aabb.max.y <= self.max.y
    }

    fn merge(&mut self, other: &Self) {
        *self = self.merged(other);
    }

    fn merged(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    fn intersects(&self, other: &Self) -> bool {
        (other.max.x >= self.min.x && other.max.y >= self.min.y) ||
            (other.min.x < self.max.x && other.min.y < self.max.y)
    }

    fn intersection_area(&self, other: &Self) -> <Self::Point as Point>::Scalar {
        self.aabb().intersection_area(&other.aabb())
    }

    fn area(&self) -> <Self::Point as Point>::Scalar {
        (self.max.x - self.min.x) * (self.max.y - self.min.y)
    }

    fn distance_2(&self, point: &Self::Point) -> <Self::Point as Point>::Scalar {
        self.aabb().distance_2(point)
    }

    fn min_max_dist_2(&self, point: &Self::Point) -> <Self::Point as Point>::Scalar {
        self.aabb().min_max_dist_2(point)
    }

    fn center(&self) -> Self::Point {
        SpatialPoint((self.min + self.max) / 2)
    }

    fn perimeter_value(&self) -> <Self::Point as Point>::Scalar {
        self.aabb().perimeter_value()
    }

    fn sort_envelopes<T: RTreeObject<Envelope=Self>>(axis: usize, envelopes: &mut [T]) {
        envelopes.sort_by_key(|o| SpatialPoint(o.envelope().min).nth(axis))
    }

    fn partition_envelopes<T: RTreeObject<Envelope=Self>>(axis: usize, envelopes: &mut [T], selection_size: usize) {
        envelopes.select_nth_unstable_by_key(selection_size, |o| SpatialPoint(o.envelope().min).nth(axis));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry<T = ()> {
    entity: Entity,
    metadata: T,
    aabb: BoundingBox,
}

impl<T> RTreeObject for Entry<T> {
    type Envelope = BoundingBox;

    fn envelope(&self) -> Self::Envelope {
        self.aabb.clone()
    }
}

impl<T> PointDistance for Entry<T> {
    fn distance_2(&self, point: &SpatialPoint) -> i32 {
        self.aabb.distance_2(point)
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

struct SelectEntityFunction<T> {
    entity: Entity,
    aabb: BoundingBox,
    _metadata: PhantomData<T>,
}

impl<T> SelectionFunction<Entry<T>> for SelectEntityFunction<T> {
    fn should_unpack_parent(&self, parent_envelope: &BoundingBox) -> bool {
        parent_envelope.contains_envelope(&self.aabb)
    }

    fn should_unpack_leaf(&self, leaf: &Entry<T>) -> bool {
        leaf.entity == self.entity
    }
}

fn clamp_point(pt: IVec2, aabb: &BoundingBox) -> IVec2 {
    let min = aabb.min;
    let max = aabb.max;
    IVec2::new(
        pt.x.max(min.x).min(max.x - 1),
        pt.y.max(min.y).min(max.y - 1),
    )
}

fn line_crosses_aabb(start: IVec2, end: IVec2, aabb: &BoundingBox) -> bool {
    let clamped_start = clamp_point(start, aabb);
    let clamped_end = clamp_point(end, aabb);
    clamped_start != clamped_end || clamped_start != start
}

struct LineSelectionFunction<T> {
    start: IVec2,
    end: IVec2,
    _metadata: PhantomData<T>,
}

impl<T> SelectionFunction<Entry<T>> for LineSelectionFunction<T> {
    fn should_unpack_parent(&self, parent_envelope: &BoundingBox) -> bool {
        line_crosses_aabb(self.start, self.end, parent_envelope)
    }

    fn should_unpack_leaf(&self, leaf: &Entry<T>) -> bool {
        line_crosses_aabb(self.start, self.end, &leaf.aabb)
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

#[derive(Debug, Clone)]
pub struct SpatialEntityTree<T = ()> {
    trees: HashMap<u8, RTree<Entry<T>>>,
    entities: HashMap<Entity, (u8, BoundingBox)>,
}

impl<T> Default for SpatialEntityTree<T> {
    fn default() -> Self {
        Self {
            trees: Default::default(),
            entities: Default::default(),
        }
    }
}

impl<T> SpatialEntityTree<T> {
    fn ensure_tree(&mut self, map: u8) -> &mut RTree<Entry<T>> {
        self.trees.entry(map)
            .or_insert_with(|| RTree::new())
    }

    fn insert(&mut self, entity: Entity, metadata: T, map: u8, aabb: BoundingBox) {
        if aabb.is_empty() {
            return;
        }

        match self.entities.get(&entity) {
            Some((old_map, old_aabb)) if (*old_map == map) && (old_aabb == &aabb) => return,
            Some(_) => self.remove(entity),
            _ => {}
        }

        self.ensure_tree(map).insert(Entry { entity, metadata, aabb });
        self.entities.insert(entity, (map, aabb));
    }

    pub fn insert_aabb(&mut self, entity: Entity, metadata: T, map: u8, min: IVec2, max: IVec2) {
        let aabb = BoundingBox::from_bounds(min, max);
        self.insert(entity, metadata, map, aabb);
    }

    pub fn insert_point(&mut self, entity: Entity, metadata: T, map: u8, position: IVec2) {
        let aabb = BoundingBox::from_point(position);
        self.insert(entity, metadata, map, aabb);
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some((map, aabb)) = self.entities.remove(&entity) {
            if let Some(tree) = self.trees.get_mut(&map) {
                tree.remove_with_selection_function(SelectEntityFunction {
                    entity,
                    aabb,
                    _metadata: Default::default(),
                });
            }
        }
    }

    pub fn iter(&self, map_id: u8) -> impl Iterator<Item=(Entity, &T, IVec2, IVec2)> + '_ {
        self.trees.get(&map_id).unwrap()
            .iter()
            .map(|e| (e.entity, &e.metadata, e.aabb.min, e.aabb.max))
    }

    pub fn iter_aabb(&self, map_id: u8, min: IVec2, max: IVec2) -> impl Iterator<Item=(Entity, &T)> + '_ {
        MaybeIter(if let Some(tree) = self.trees.get(&map_id) {
            let envelope = BoundingBox::from_bounds(min, max);
            Some(tree.locate_in_envelope_intersecting(&envelope)
                .map(|e| (e.entity, &e.metadata)))
        } else {
            None
        })
    }

    pub fn iter_line(&self, map_id: u8, start: IVec2, end: IVec2) -> impl Iterator<Item=(Entity, &T)> + '_ {
        MaybeIter(if let Some(tree) = self.trees.get(&map_id) {
            let function = LineSelectionFunction {
                start,
                end,
                _metadata: PhantomData,
            };
            Some(tree.locate_with_selection_function(function)
                .map(|e| (e.entity, &e.metadata)))
        } else {
            None
        })
    }

    pub fn iter_at_point(&self, map: u8, position: IVec2) -> impl Iterator<Item=(Entity, &T)> + '_ {
        MaybeIter(if let Some(tree) = self.trees.get(&map) {
            Some(tree.locate_all_at_point(&SpatialPoint(position))
                .map(|e| (e.entity, &e.metadata)))
        } else {
            None
        })
    }
}

#[derive(Debug, Clone)]
pub enum SurfaceKind {
    Chunk { position: IVec2, chunk: MapChunk },
    Item { position: IVec2, tile_id: u16, impassable: bool, min_z: i32, max_z: i32 },
}

#[derive(Debug, Clone, Default, Resource)]
pub struct EntitySurfaces {
    pub tree: SpatialEntityTree<SurfaceKind>,
}

pub fn update_entity_surfaces(
    mut storage: ResMut<EntitySurfaces>,
    tile_data: Res<TileDataResource>,
    chunks: Query<(Entity, &Location, &Chunk), Or<(Changed<Location>, Changed<Chunk>)>>,
    surfaces: Query<
        (Entity, &Location, &Graphic, Option<&Impassable>),
        (Or<(With<Impassable>, With<Surface>)>, Or<(Changed<Location>, Changed<Surface>, Changed<Impassable>)>),
    >,
    mut removed_chunks: RemovedComponents<Chunk>,
    mut removed_surfaces: RemovedComponents<Surface>,
) {
    for (entity, position, chunk) in chunks.iter() {
        let min = position.position.truncate();
        let max = min + IVec2::new(CHUNK_SIZE as i32, CHUNK_SIZE as i32);
        let kind = SurfaceKind::Chunk {
            position: min,
            chunk: chunk.map_chunk.clone(),
        };
        storage.tree.insert_aabb(entity, kind, position.map_id, min, max);
    }

    for (entity, position, graphic, impassable) in surfaces.iter() {
        let tile_data = match tile_data.items.get(graphic.id as usize) {
            Some(x) => x,
            None => continue,
        };

        let kind = SurfaceKind::Item {
            position: position.position.truncate(),
            tile_id: graphic.id,
            impassable: impassable.is_some(),
            min_z: position.position.z,
            max_z: position.position.z + tile_data.height as i32,
        };
        storage.tree.insert_point(entity, kind, position.map_id, position.position.truncate());
    }

    for entity in removed_chunks.iter() {
        storage.tree.remove(entity);
    }

    for entity in removed_surfaces.iter() {
        storage.tree.remove(entity);
    }
}

#[derive(Debug, Clone, Component)]
pub struct Extents {
    min: IVec3,
    max: IVec3,
}

impl Extents {
    pub const ONE: Extents = Self { min: IVec3::ZERO, max: IVec3::ONE };
}

impl Default for Extents {
    fn default() -> Self {
        Self::ONE
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct EntityPositions {
    pub tree: SpatialEntityTree,
}

pub fn update_entity_positions(
    mut storage: ResMut<EntityPositions>,
    entities: Query<(Entity, &Location, Option<&Extents>), Or<(Changed<Location>, Changed<Extents>)>>,
    mut removed_entities: RemovedComponents<Location>,
) {
    for (entity, position, extents) in &entities {
        let extents = extents.cloned().unwrap_or(Extents::default());
        let map_id = position.map_id;
        let position = position.position.truncate();
        let min = position - extents.min.truncate();
        let max = position + extents.max.truncate();
        storage.tree.insert_aabb(entity, (), map_id, min, max);
    }

    for entity in removed_entities.iter() {
        storage.tree.remove(entity);
    }
}

pub fn view_aabb(position: IVec2, range: i32) -> (IVec2, IVec2) {
    let size = IVec2::splat(range);
    let min = position - size;
    let max = position + size + IVec2::ONE;
    (min, max)
}

#[derive(Debug, Clone, Default, Resource)]
pub struct NetClientPositions {
    pub tree: SpatialEntityTree,
}

pub fn update_client_positions(
    mut storage: ResMut<NetClientPositions>,
    clients: Query<(Entity, &View, &Possessing), With<NetClient>>,
    characters: Query<&Location>,
    changed_clients: Query<
        (Entity, &View, &Possessing),
        (With<NetClient>, Or<(Changed<View>, Changed<Possessing>)>),
    >,
    changed_characters: Query<(Entity, &NetOwner, &Location), Changed<Location>>,
    mut removed_clients: RemovedComponents<NetClient>,
) {
    for (entity, view, possessing) in changed_clients.iter() {
        let map_position = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        let position = map_position.position.truncate();
        let (min, max) = view_aabb(position, view.range);
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

        let position = map_position.position.truncate();
        let (min, max) = view_aabb(position, view.range);
        storage.tree.insert_aabb(entity, (), map_position.map_id, min, max);
    }

    for entity in removed_clients.iter() {
        storage.tree.remove(entity);
    }
}
