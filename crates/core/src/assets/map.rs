use std::path::Path;

use byteorder::{LittleEndian as Endian, ReadBytesExt};

use crate::assets::mul::MulReader;

pub struct MapTiles {
    pub tile_ids: Vec<u16>,
    pub heights: Vec<u8>,
}

pub async fn load_map(data_path: &Path, index: usize, width: usize, height: usize) -> anyhow::Result<MapTiles> {
    let mut reader = MulReader::open(data_path, &format!("map{}", index)).await?;
    let mut tile_ids = vec![0u16; width * height];
    let mut heights = vec![0u8; width * height];

    let height_blocks = height / 16;

    loop {
        let block_index = match reader.read_u32::<Endian>() {
            Ok(x) => x as usize,
            Err(_) => break,
        };

        let block_x = (block_index / height_blocks) * 16;
        let block_y = (block_index % height_blocks) * 16;

        if block_x >= width || block_y >= height {
            continue;
        }

        for tile_index in 0..64 {
            let x = block_x + (tile_index & 0xf);
            let y = block_y + (tile_index / 16);
            let index = x + y * width;

            let tile_id = match reader.read_u16::<Endian>() {
                Ok(x) => x,
                Err(_) => break,
            };
            let z = reader.read_u8()?;

            tile_ids[index] = tile_id;
            heights[index] = z;
        }
    }

    log::debug!("map {} - {} tiles", index, tile_ids.len());
    Ok(MapTiles {
        tile_ids,
        heights,
    })
}
