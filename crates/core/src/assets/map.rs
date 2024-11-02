use std::io::Read;
use std::path::Path;

use bytemuck::{Pod, Zeroable};
use byteorder::{LittleEndian as Endian, ReadBytesExt};
use glam::IVec3;

use crate::assets::mul::MulReader;

pub const CHUNK_SIZE: usize = 8;
pub const CHUNK_AREA: usize = CHUNK_SIZE * CHUNK_SIZE;

#[derive(Debug, Clone, Copy, Default)]
pub struct MapTile {
    pub tile_id: u16,
    pub height: i8,
}

#[derive(Debug, Clone)]
pub struct MapChunk {
    pub tiles: [MapTile; CHUNK_AREA],
}

impl Default for MapChunk {
    fn default() -> Self {
        MapChunk {
            tiles: [MapTile { tile_id: 0, height: 0 }; CHUNK_AREA],
        }
    }
}

impl MapChunk {
    pub fn get(&self, x: usize, y: usize) -> MapTile {
        let index = x + CHUNK_SIZE * y;
        self.tiles[index]
    }
}

pub fn map_chunk_count(width: usize, height: usize) -> (usize, usize) {
    (width.div_ceil(CHUNK_SIZE), height.div_ceil(CHUNK_SIZE))
}

#[derive(Clone, Copy, Default, Zeroable, Pod)]
#[repr(C, packed)]
struct RawTile {
    pub tile_id: u16,
    pub height: i8,
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C, packed)]
struct RawChunk {
    pub unused: u32,
    pub tiles: [RawTile; CHUNK_AREA],
}

pub async fn load_map(
    data_path: &Path,
    index: usize,
    width: usize,
    height: usize,
    mut callback: impl FnMut(usize, usize, MapChunk) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut reader = MulReader::open(data_path, &format!("map{}", index)).await?;

    let (width_chunks, height_chunks) = map_chunk_count(width, height);
    let num_chunks = width_chunks * height_chunks;
    let num_bytes = size_of::<RawChunk>() * num_chunks;
    let mut bytes = vec![0; num_bytes];
    reader.read_exact(&mut bytes)?;
    let mut chunks: &[RawChunk] = bytemuck::cast_slice(&bytes);

    for x in 0..width_chunks {
        for y in 0..height_chunks {
            let (in_chunk, next) = chunks.split_first().unwrap();
            chunks = next;

            let mut out_chunk = MapChunk::default();

            for (in_tile, out_tile) in in_chunk.tiles.iter().zip(out_chunk.tiles.iter_mut()) {
                out_tile.tile_id = in_tile.tile_id;
                out_tile.height = in_tile.height;
            }

            callback(x, y, out_chunk)?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Static {
    pub position: IVec3,
    pub graphic_id: u16,
    pub hue: u16,
}

#[derive(Clone, Copy, Default, Zeroable, Pod)]
#[repr(C, packed)]
struct RawStatic {
    pub graphic_id: u16,
    pub x_off: i8,
    pub y_off: i8,
    pub z: i8,
    pub hue: u16,
}

pub trait StaticVisitor {
    fn start_chunk(&mut self, num: usize) -> anyhow::Result<()>;

    fn item(&mut self, item: Static) -> anyhow::Result<()>;
}

pub async fn load_statics(
    data_path: &Path,
    index: usize,
    width: usize,
    height: usize,
    visitor: &mut impl StaticVisitor,
) -> anyhow::Result<()> {
    let mut index_reader = MulReader::open(data_path, &format!("staidx{}", index)).await?;
    let data = {
        let mut data = Vec::new();
        let mut data_reader = MulReader::open(data_path, &format!("statics{}", index)).await?;
        data_reader.read_to_end(&mut data)?;
        data
    };
    
    let (width_blocks, height_blocks) = map_chunk_count(width, height);
    for block_x in 0..width_blocks {
        for block_y in 0..height_blocks {
            let offset = index_reader.read_u32::<Endian>()?;
            let length = index_reader.read_u32::<Endian>()?;
            index_reader.read_u32::<Endian>()?;
            if offset == 0xffffffff || length == 0xffffffff {
                continue;
            }

            let x_base = (block_x * CHUNK_SIZE) as i32;
            let y_base = (block_y * CHUNK_SIZE) as i32;

            let start = offset as usize;
            let end = start + (length as usize);

            let bytes = &data[start..end];
            let statics: &[RawStatic] = bytemuck::cast_slice(bytes);

            visitor.start_chunk(statics.len())?;
            for item in statics {
                let x = x_base + item.x_off as i32;
                let y = y_base + item.y_off as i32;
                visitor.item(Static {
                    position: IVec3::new(x, y, item.z as i32),
                    graphic_id: item.graphic_id,
                    hue: item.hue,
                })?;
            }
        }
    }

    Ok(())
}
