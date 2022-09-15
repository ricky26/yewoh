use std::future::Future;
use std::io::{ErrorKind, Read};
use std::path::Path;

use byteorder::{LittleEndian as Endian, ReadBytesExt};
use glam::IVec3;

use crate::assets::mul::MulReader;

#[derive(Debug, Clone)]
pub struct MapChunk {
    pub tile_ids: [u16; 64],
    pub heights: [u8; 64],
}

impl Default for MapChunk {
    fn default() -> Self {
        MapChunk {
            tile_ids: [0; 64],
            heights: [0; 64],
        }
    }
}

impl MapChunk {
    pub fn get(&self, x: usize, y: usize) -> (u16, u8) {
        let index = x + 16 * y;
        (self.tile_ids[index], self.heights[index])
    }
}

pub async fn load_map<C: FnMut(usize, MapChunk) -> F, F: Future<Output=anyhow::Result<()>>>(
    data_path: &Path,
    index: usize,
    width: usize,
    height: usize,
    mut callback: C,
) -> anyhow::Result<()> {
    let mut reader = MulReader::open(data_path, &format!("map{}", index)).await?;
    let height_blocks = height / 16;

    loop {
        let block_index = match reader.read_u32::<Endian>() {
            Ok(x) => x as usize,
            Err(err) if err.kind() == ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        };

        let block_x = (block_index / height_blocks) * 16;
        let block_y = (block_index % height_blocks) * 16;

        if block_x >= width || block_y >= height {
            continue;
        }

        let mut chunk = MapChunk::default();
        for tile_index in 0..64 {
            chunk.tile_ids[tile_index] = reader.read_u16::<Endian>()?;
            chunk.heights[tile_index] = reader.read_u8()?;
        }

        (callback)(block_index, chunk).await?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Static {
    pub position: IVec3,
    pub graphic_id: u16,
    pub hue: u16,
}

pub async fn load_statics<C: FnMut(Static) -> F, F: Future<Output=anyhow::Result<()>>>(
    data_path: &Path,
    index: usize,
    width: usize,
    height: usize,
    mut callback: C,
) -> anyhow::Result<()> {
    let mut index_reader = MulReader::open(data_path, &format!("staidx{}", index)).await?;
    let data = {
        let mut data = Vec::new();
        let mut data_reader = MulReader::open(data_path, &format!("statics{}", index)).await?;
        data_reader.read_to_end(&mut data)?;
        data
    };

    let width_blocks = (width + 15) / 16;
    let height_blocks = (height + 15) / 16;

    for block_x in 0..width_blocks {
        for block_y in 0..height_blocks {
            let offset = index_reader.read_u32::<Endian>()?;
            let length = index_reader.read_u32::<Endian>()?;
            index_reader.read_u32::<Endian>()?;
            if offset == 0xffffffff || length == 0xffffffff {
                continue;
            }

            let x_base = (block_x * 16) as i32;
            let y_base = (block_y * 16) as i32;

            let start = offset as usize;
            let end = start + (length as usize);

            let mut bytes = &data[start..end];

            while bytes.len() > 0 {
                let graphic_id = bytes.read_u16::<Endian>()?;
                let x_off = bytes.read_i8()? as i32;
                let y_off = bytes.read_i8()? as i32;
                let z = bytes.read_i8()? as i32;
                let hue = bytes.read_u16::<Endian>()?;

                let x = x_base + x_off;
                let y = y_base + y_off;
                (callback)(Static {
                    position: IVec3::new(x, y, z),
                    graphic_id,
                    hue,
                }).await?;
            }
        }
    }

    Ok(())
}
