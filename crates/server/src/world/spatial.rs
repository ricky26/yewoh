use std::collections::hash_map::Entry;
use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use glam::{ivec2, IVec2};
use smallvec::SmallVec;
use yewoh::assets::map::CHUNK_SIZE;

use crate::world::characters::CharacterBodyType;
use crate::world::entity::MapPosition;
use crate::world::items::ItemGraphic;
use crate::world::map::{Chunk, MapInfos, Static, TileDataResource};
use crate::world::ServerSet;

fn cell_index(size: IVec2, position: IVec2) -> Option<usize> {
    if position.x < 0 ||
        position.x >= size.x ||
        position.y < 0 ||
        position.y >= size.y {
        None
    } else {
        Some((position.x as usize) + (position.y as usize) * (size.x as usize))
    }
}

pub trait BucketEntry {
    fn entity(&self) -> Entity;

    fn before(&self, other: &Self) -> bool;
}

#[derive(Debug, Clone, Reflect)]
pub struct SortedBucket<const N: usize, T: BucketEntry> {
    pub entries: SmallVec<[T; N]>,
}

impl<const N: usize, T: BucketEntry> Default for SortedBucket<N, T> {
    fn default() -> Self {
        SortedBucket {
            entries: SmallVec::new(),
        }
    }
}

impl<const N: usize, T: BucketEntry> SortedBucket<N, T> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[T] {
        self.entries.as_slice()
    }

    pub fn iter(&self) -> std::slice::Iter<T> {
        self.entries.iter()
    }

    pub fn insert(&mut self, entry: T) {
        let insert_index = self.entries.iter()
            .position(|e| entry.before(e))
            .unwrap_or_else(|| self.entries.len());
        self.entries.insert(insert_index, entry);
    }

    pub fn remove(&mut self, entity: Entity) {
        let index = self.entries.iter()
            .position(|e| e.entity() == entity)
            .unwrap();
        self.entries.remove(index);
    }
}

#[derive(Debug, Clone, Reflect)]
pub struct BucketMap<const N: usize, T: BucketEntry> {
    pub size: IVec2,
    pub buckets: Vec<SortedBucket<N, T>>,
}

impl<const N: usize, T: BucketEntry> Default for BucketMap<N, T> {
    fn default() -> Self {
        BucketMap {
            size: IVec2::ZERO,
            buckets: Vec::new(),
        }
    }
}

impl<const N: usize, T: BucketEntry> BucketMap<N, T> {
    fn bucket_index(&self, position: IVec2) -> Option<usize> {
        cell_index(self.size, position)
    }

    pub fn new(size: IVec2) -> BucketMap<N, T> {
        let capacity = (size.x as usize) * (size.y as usize);
        let mut buckets = Vec::with_capacity(capacity);
        buckets.resize_with(capacity, SortedBucket::default);
        BucketMap {
            size,
            buckets,
        }
    }

    pub fn entries_at(&self, position: IVec2) -> &[T] {
        if let Some(bucket_index) = self.bucket_index(position) {
            let bucket = &self.buckets[bucket_index];
            bucket.entries()
        } else {
            &[]
        }
    }

    pub fn insert(&mut self, position: IVec2, entry: T) {
        if let Some(bucket_index) = self.bucket_index(position) {
            let bucket = &mut self.buckets[bucket_index];
            bucket.insert(entry);
        }
    }

    pub fn remove(&mut self, entity: Entity, position: IVec2) {
        if let Some(bucket_index) = self.bucket_index(position) {
            let bucket = &mut self.buckets[bucket_index];
            bucket.remove(entity);
        }
    }
}

#[derive(Debug, Clone, Reflect)]
pub struct MapBucketMap<const N: usize, T: BucketEntry> {
    pub maps: HashMap<u8, BucketMap<N, T>>,
    pub entities: HashMap<Entity, (u8, IVec2)>,
}

impl<const N: usize, T: BucketEntry> Default for MapBucketMap<N, T> {
    fn default() -> Self {
        MapBucketMap {
            maps: HashMap::default(),
            entities: HashMap::default(),
        }
    }
}

impl<const N: usize, T: BucketEntry> MapBucketMap<N, T> {
    pub fn new(map_infos: &MapInfos) -> MapBucketMap<N, T> {
        let mut ret = MapBucketMap::default();

        for (map_id, map) in &map_infos.maps {
            ret.insert_map(*map_id, map.size.as_ivec2());
        }

        ret
    }

    pub fn insert_map(&mut self, map_id: u8, size: IVec2) {
        self.maps.insert(map_id, BucketMap::new(size));
    }

    pub fn entries_at(&self, map_id: u8, position: IVec2) -> &[T] {
        if let Some(map) = self.maps.get(&map_id) {
            map.entries_at(position)
        } else {
            &[]
        }
    }

    pub fn insert(&mut self, map_id: u8, position: IVec2, entry: T) -> bool {
        let entity = entry.entity();
        let map = match self.entities.entry(entity) {
            Entry::Occupied(mut existing) => {
                let (old_map_id, old_position) = existing.get_mut();
                let old_map = self.maps.get_mut(old_map_id).unwrap();
                old_map.remove(entity, *old_position);

                if *old_map_id == map_id {
                    *old_map_id = map_id;
                    *old_position = position;
                    old_map
                } else if let Some(map) = self.maps.get_mut(&map_id) {
                    *old_map_id = map_id;
                    *old_position = position;
                    map
                } else {
                    return false;
                }
            }
            Entry::Vacant(entry) => {
                let Some(map) = self.maps.get_mut(&map_id) else {
                    return false;
                };
                entry.insert((map_id, position));
                map
            }
        };

        map.insert(position, entry);
        true
    }

    pub fn remove(&mut self, entity: Entity) -> bool {
        if let Some((old_map_id, old_position)) = self.entities.remove(&entity) {
            let map = self.maps.get_mut(&old_map_id).unwrap();
            map.remove(entity, old_position);
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Reflect)]
pub struct CharacterEntry {
    pub entity: Entity,
    pub z: i32,
}

impl BucketEntry for CharacterEntry {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn before(&self, other: &Self) -> bool {
        self.z < other.z
    }
}

#[derive(Debug, Clone, Default, Reflect, Resource)]
#[reflect(Resource)]
pub struct SpatialCharacterLookup {
    pub lookup: MapBucketMap<2, CharacterEntry>,
}

impl SpatialCharacterLookup {
    pub fn new(map_infos: &MapInfos) -> SpatialCharacterLookup {
        SpatialCharacterLookup {
            lookup: MapBucketMap::new(map_infos),
        }
    }
}

pub fn update_character_lookup(
    mut lookup: ResMut<SpatialCharacterLookup>,
    entities: Query<(Entity, &MapPosition), (With<CharacterBodyType>, Changed<MapPosition>)>,
    mut removed_entities: RemovedComponents<MapPosition>,
) {
    for (entity, position) in &entities {
        lookup.lookup.insert(position.map_id, position.position.truncate(), CharacterEntry {
            entity,
            z: position.position.z,
        });
    }

    for entity in removed_entities.read() {
        lookup.lookup.remove(entity);
    }
}

#[derive(Debug, Clone, Reflect)]
pub struct ItemEntry {
    pub entity: Entity,
    pub z_min: i32,
    pub z_max: i32,
    pub graphic: u16,
}

impl BucketEntry for ItemEntry {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn before(&self, other: &Self) -> bool {
        self.z_min < other.z_min
    }
}

#[derive(Clone, Debug, Default, Reflect, Resource)]
#[reflect(Resource)]
pub struct SpatialDynamicItemLookup {
    pub lookup: MapBucketMap<2, ItemEntry>,
}

impl SpatialDynamicItemLookup {
    pub fn new(map_infos: &MapInfos) -> SpatialDynamicItemLookup {
        SpatialDynamicItemLookup {
            lookup: MapBucketMap::new(map_infos),
        }
    }
}

pub fn update_dynamic_item_lookup(
    mut lookup: ResMut<SpatialDynamicItemLookup>,
    tile_data: Res<TileDataResource>,
    surfaces: Query<
        (Entity, &MapPosition, &ItemGraphic),
        (Without<Static>, Or<(Changed<MapPosition>, Changed<ItemGraphic>)>),
    >,
    mut removed: RemovedComponents<MapPosition>,
) {
    for (entity, position, graphic) in surfaces.iter() {
        let tile_data = match tile_data.items.get(**graphic as usize) {
            Some(x) => x,
            None => continue,
        };

        lookup.lookup.insert(position.map_id, position.position.truncate(), ItemEntry {
            entity,
            z_min: position.position.z,
            z_max: position.position.z + (tile_data.height as i32),
            graphic: **graphic,
        });
    }

    for entity in removed.read() {
        lookup.lookup.remove(entity);
    }
}

#[derive(Clone, Debug, Default, Reflect, Resource)]
#[reflect(Resource)]
pub struct SpatialStaticItemLookup {
    pub lookup: MapBucketMap<2, ItemEntry>,
}

impl SpatialStaticItemLookup {
    pub fn new(map_infos: &MapInfos) -> SpatialStaticItemLookup {
        SpatialStaticItemLookup {
            lookup: MapBucketMap::new(map_infos),
        }
    }
}

#[derive(Component)]
pub struct ProcessedStatic;

pub fn update_static_item_lookup(
    mut commands: Commands,
    mut lookup: ResMut<SpatialStaticItemLookup>,
    tile_data: Res<TileDataResource>,
    surfaces: Query<
        (Entity, &MapPosition, &ItemGraphic),
        (With<Static>, Without<ProcessedStatic>),
    >,
) {
    for (entity, position, graphic) in surfaces.iter() {
        commands.entity(entity).insert(ProcessedStatic);

        let tile_data = match tile_data.items.get(**graphic as usize) {
            Some(x) => x,
            None => continue,
        };
        lookup.lookup.insert(position.map_id, position.position.truncate(), ItemEntry {
            entity,
            z_min: position.position.z,
            z_max: position.position.z + (tile_data.height as i32),
            graphic: **graphic,
        });
    }
}

#[derive(Debug, Clone, Default, Reflect)]
pub struct MapChunkLookup {
    pub size: IVec2,
    pub chunks: Vec<Option<Entity>>,
}

impl MapChunkLookup {
    pub fn chunk_index(&self, position: IVec2) -> Option<usize> {
        cell_index(self.size, position)
    }

    pub fn new(size: IVec2) -> MapChunkLookup {
        let width = (size.x as usize).div_ceil(CHUNK_SIZE);
        let height = (size.y as usize).div_ceil(CHUNK_SIZE);
        let size = ivec2(width as i32, height as i32);
        let mut chunks = Vec::new();
        chunks.resize_with(width * height, || None);
        MapChunkLookup { size, chunks }
    }

    pub fn get_at(&self, position: IVec2) -> Option<Entity> {
        let chunk_pos = position / (CHUNK_SIZE as i32);
        if let Some(index) = self.chunk_index(chunk_pos) {
            self.chunks[index]
        } else {
            None
        }
    }

    pub fn insert(&mut self, position: IVec2, entity: Entity) {
        let chunk_pos = position / (CHUNK_SIZE as i32);
        if let Some(index) = self.chunk_index(chunk_pos) {
            self.chunks[index] = Some(entity);
        }
    }
}

#[derive(Debug, Clone, Default, Reflect, Resource)]
#[reflect(Resource)]
pub struct ChunkLookup {
    pub maps: HashMap<u8, MapChunkLookup>,
}

impl ChunkLookup {
    pub fn new(map_infos: &MapInfos) -> ChunkLookup {
        let mut ret = ChunkLookup::default();

        for (map_id, map) in &map_infos.maps {
            ret.insert_map(*map_id, map.size.as_ivec2());
        }

        ret
    }

    pub fn insert_map(&mut self, map_id: u8, size: IVec2) {
        self.maps.insert(map_id, MapChunkLookup::new(size));
    }

    pub fn get_at(&self, map_id: u8, position: IVec2) -> Option<Entity> {
        if let Some(map) = self.maps.get(&map_id) {
            map.get_at(position)
        } else {
            None
        }
    }

    pub fn insert(&mut self, map_id: u8, position: IVec2, entity: Entity) {
        if let Some(map) = self.maps.get_mut(&map_id) {
            map.insert(position, entity);
        }
    }
}

pub fn update_chunk_lookup(
    mut commands: Commands,
    mut lookup: ResMut<ChunkLookup>,
    chunks: Query<
        (Entity, &MapPosition),
        (With<Static>, With<Chunk>, Without<ProcessedStatic>),
    >,
) {
    for (entity, position) in chunks.iter() {
        commands.entity(entity).insert(ProcessedStatic);
        lookup.insert(position.map_id, position.position.truncate(), entity);
    }
}

pub enum Collider {
    Chunk(Entity),
    StaticItem(ItemEntry),
    DynamicItem(ItemEntry),
}

impl Collider {
    pub fn entity(&self) -> Entity {
        match self {
            Collider::Chunk(entity) => *entity,
            Collider::StaticItem(entry) => entry.entity,
            Collider::DynamicItem(entry) => entry.entity,
        }
    }
}

fn slice_pop_last<T>(slice: &mut &[T]) {
    *slice = &slice[..slice.len() - 1];
}

fn slice_pop_first<'a, T>(slice: &mut &'a [T]) -> Option<&'a T> {
    if let Some((result, rest)) = slice.split_first() {
        *slice = rest;
        Some(result)
    } else {
        None
    }
}

pub struct ColliderIter<'a> {
    chunk: Option<Entity>,
    static_items: &'a [ItemEntry],
    dynamic_items: &'a [ItemEntry],
}

impl Iterator for ColliderIter<'_> {
    type Item = Collider;

    fn next(&mut self) -> Option<Self::Item> {
        let next_static = self.static_items.last();
        let next_dynamic = self.dynamic_items.last();

        if let (Some(next_static), Some(next_dynamic)) = (next_static, next_dynamic) {
            if next_static.z_min > next_dynamic.z_min {
                slice_pop_last(&mut self.static_items);
                Some(Collider::StaticItem(next_static.clone()))
            } else {
                slice_pop_last(&mut self.dynamic_items);
                Some(Collider::DynamicItem(next_dynamic.clone()))
            }
        } else if let Some(next_dynamic) = next_dynamic {
            slice_pop_last(&mut self.dynamic_items);
            Some(Collider::DynamicItem(next_dynamic.clone()))
        } else if let Some(next_static) = next_static {
            slice_pop_last(&mut self.static_items);
            Some(Collider::StaticItem(next_static.clone()))
        } else if let Some(entity) = self.chunk {
            self.chunk = None;
            Some(Collider::Chunk(entity))
        } else {
            None
        }
    }
}

pub struct SpatialIter<'a> {
    chunk: Option<Entity>,
    static_items: &'a [ItemEntry],
    dynamic_items: &'a [ItemEntry],
    characters: &'a [CharacterEntry],
}

impl Iterator for SpatialIter<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(entry) = slice_pop_first(&mut self.characters) {
            Some(entry.entity)
        } else if let Some(entry) = slice_pop_first(&mut self.dynamic_items) {
            Some(entry.entity)
        } else if let Some(entry) = slice_pop_first(&mut self.static_items) {
            Some(entry.entity)
        } else {
            self.chunk.take()
        }
    }
}

#[derive(SystemParam)]
pub struct SpatialQuery<'w> {
    pub characters: Res<'w, SpatialCharacterLookup>,
    pub dynamic_items: Res<'w, SpatialDynamicItemLookup>,
    pub static_items: Res<'w, SpatialStaticItemLookup>,
    pub chunks: Res<'w, ChunkLookup>,
}

impl SpatialQuery<'_> {
    pub fn iter_colliders(&self, map_id: u8, position: IVec2) -> ColliderIter {
        let dynamic_items = self.dynamic_items.lookup.entries_at(map_id, position);
        let static_items = self.static_items.lookup.entries_at(map_id, position);
        let chunk = self.chunks.get_at(map_id, position);
        ColliderIter {
            chunk,
            static_items,
            dynamic_items,
        }
    }

    pub fn iter_at(&self, map_id: u8, position: IVec2) -> SpatialIter {
        let characters = self.characters.lookup.entries_at(map_id, position);
        let dynamic_items = self.dynamic_items.lookup.entries_at(map_id, position);
        let static_items = self.static_items.lookup.entries_at(map_id, position);
        let chunk = self.chunks.get_at(map_id, position);
        SpatialIter {
            chunk,
            static_items,
            dynamic_items,
            characters,
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .init_resource::<SpatialCharacterLookup>()
        .init_resource::<SpatialDynamicItemLookup>()
        .init_resource::<SpatialStaticItemLookup>()
        .init_resource::<ChunkLookup>()
        .add_systems(PostUpdate, (
            update_character_lookup,
            update_dynamic_item_lookup,
            update_static_item_lookup,
            update_chunk_lookup,
        ).in_set(ServerSet::UpdateVisibility));
}
