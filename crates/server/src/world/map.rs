use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;

use bevy::prelude::*;
use glam::IVec3;
use tokio::task::JoinSet;
use yewoh::assets::map::{load_map, load_statics, map_chunk_count, MapChunk, StaticVisitor, CHUNK_SIZE};
use yewoh::assets::multi::MultiData;
use yewoh::assets::tiles::{TileData, TileFlags};
use yewoh::Direction;

use crate::world::entity::{Hue, MapPosition};
use crate::world::items::ItemGraphic;

#[derive(Debug, Clone, Default, Reflect)]
#[reflect(Default)]
pub struct MapInfo {
    pub size: UVec2,
    pub season: u8,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Default, Reflect, Resource)]
#[reflect(Default, Resource)]
pub struct MapInfos {
    pub maps: HashMap<u8, MapInfo>,
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct Chunk {
    #[reflect(ignore)]
    pub map_chunk: MapChunk,
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct Static;

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Default, Component)]
pub struct HasCollision;

#[derive(Debug, Clone, Default, Resource, Reflect)]
#[reflect(Resource)]
pub struct TileDataResource {
    #[reflect(ignore)]
    pub tile_data: TileData,
}

impl Deref for TileDataResource {
    type Target = TileData;

    fn deref(&self) -> &Self::Target {
        &self.tile_data
    }
}

#[derive(Debug, Clone, Default, Resource, Reflect)]
#[reflect(Resource)]
pub struct MultiDataResource {
    #[reflect(ignore)]
    pub multi_data: MultiData,
}

impl Deref for MultiDataResource {
    type Target = MultiData;

    fn deref(&self) -> &Self::Target {
        &self.multi_data
    }
}

pub struct MapChunkData {
    pub map_id: u8,
    pub x: usize,
    pub y: usize,
    pub chunk: MapChunk,
}

pub async fn load_map_entities(
    map_infos: &MapInfos,
    uo_data_path: &Path,
) -> anyhow::Result<Vec<MapChunkData>> {
    let mut set = JoinSet::new();

    for (map_id, map) in map_infos.maps.iter() {
        if map.is_virtual {
            continue;
        }

        let map_id = *map_id;
        let width = map.size.x as usize;
        let height = map.size.y as usize;
        let (width_chunks, height_chunks) = map_chunk_count(width, height);
        let num_chunks = width_chunks * height_chunks;
        let uo_data_path = uo_data_path.to_path_buf();
        set.spawn(async move {
            let mut data = Vec::with_capacity(num_chunks);
            load_map(&uo_data_path, map_id as usize, width, height, |x, y, chunk| {
                data.push(MapChunkData {
                    map_id,
                    x,
                    y,
                    chunk,
                });
                Ok(())
            }).await?;
            Ok::<_, anyhow::Error>(data)
        });
    }

    let mut data = Vec::new();
    while let Some(result) = set.join_next().await {
        let item = result??;
        data.extend(item);
    }
    Ok(data)
}

pub fn spawn_map_entities(
    world: &mut World,
    map_data: impl Iterator<Item=MapChunkData>,
) {
    world.spawn_batch(map_data.map(|chunk| {
        let x = (chunk.x * CHUNK_SIZE) as i32;
        let y = (chunk.y * CHUNK_SIZE) as i32;
        let position = IVec3::new(x, y, 0);
        (
            Chunk { map_chunk: chunk.chunk },
            MapPosition { map_id: chunk.map_id, position, direction: Direction::default() },
            Static,
        )
    }));
}

#[derive(Clone, Debug)]
pub struct StaticData {
    pub map_id: u8,
    pub position: IVec3,
    pub graphic_id: u16,
    pub hue: u16,
}

pub async fn load_static_entities(
    map_infos: &MapInfos,
    uo_data_path: &Path,
) -> anyhow::Result<Vec<StaticData>> {
    struct Visit {
        map_id: u8,
        statics: Vec<StaticData>,
    }

    impl StaticVisitor for Visit {
        fn start_chunk(&mut self, num: usize) -> anyhow::Result<()> {
            self.statics.reserve(num);
            Ok(())
        }

        fn item(&mut self, item: yewoh::assets::map::Static) -> anyhow::Result<()> {
            self.statics.push(StaticData {
                map_id: self.map_id,
                position: item.position,
                graphic_id: item.graphic_id,
                hue: item.hue,
            });
            Ok(())
        }
    }

    let mut set = JoinSet::new();

    for (map_id, map) in map_infos.maps.iter() {
        if map.is_virtual {
            continue;
        }

        let map_id = *map_id;
        let width = map.size.x as usize;
        let height = map.size.y as usize;
        let uo_data_path = uo_data_path.to_path_buf();
        set.spawn(async move {
            let mut visitor = Visit {
                map_id,
                statics: Vec::new(),
            };
            load_statics(&uo_data_path, map_id as usize, width, height, &mut visitor).await?;
            Ok::<_, anyhow::Error>(visitor.statics)
        });
    }

    let mut data = Vec::new();
    while let Some(result) = set.join_next().await {
        let item = result??;
        data.extend(item);
    }
    Ok(data)
}

const HAS_COLLISION_FLAGS: TileFlags = TileFlags::from_bits_truncate(TileFlags::SURFACE.bits() |
    TileFlags::IMPASSABLE.bits() |
    TileFlags::WALL.bits() |
    TileFlags::BRIDGE.bits());

pub fn spawn_static_entities(
    world: &mut World,
    tile_data: &TileData,
    statics: &[StaticData],
) {
    let hint = statics.len() / 2;
    let mut out_collision = Vec::with_capacity(hint);

    let entities = world.spawn_batch(statics.iter()
        .map(|item| (
            MapPosition {
                map_id: item.map_id,
                position: item.position,
                direction: Direction::default()
            },
            ItemGraphic(item.graphic_id),
            Hue(item.hue),
            Static,
        )));

    for (entity, item) in entities.zip(statics.iter()) {
        if let Some(info) = tile_data.items.get(item.graphic_id as usize) {
            if info.flags.intersects(HAS_COLLISION_FLAGS) {
                out_collision.push((entity, HasCollision));
            }
        }
    }

    world.insert_batch(out_collision);
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<MapInfo>()
        .register_type::<MapInfos>()
        .register_type::<Chunk>()
        .register_type::<Static>()
        .register_type::<HasCollision>()
        .register_type::<TileDataResource>()
        .init_resource::<MapInfos>()
        .init_resource::<TileDataResource>();
}
