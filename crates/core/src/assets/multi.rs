use std::path::Path;

use byteorder::{LittleEndian as Endian, ReadBytesExt};
use glam::IVec3;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::assets::tiles::TileFlags;
use crate::assets::uop::UopBuffer;

const MAX_MULTIS: usize = 9000;

#[derive(Debug, Clone)]
pub struct MultiPrefabComponent {
    pub graphic: u16,
    pub position: IVec3,
    pub tile_flags: TileFlags,
    pub component_flags: u16,
    pub tooltip_ids: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct MultiPrefab {
    pub components: Vec<MultiPrefabComponent>,
}

#[derive(Debug, Clone, Default)]
pub struct MultiData {
    pub prefabs: Vec<MultiPrefab>,
}

pub async fn load_multi_data(data_path: &Path) -> anyhow::Result<MultiData> {
    let uop_path = data_path.join("MultiCollection.uop");
    let mut file = File::open(&uop_path).await?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).await?;
    let uop = UopBuffer::try_from_backing(contents)?;
    let mut prefabs = Vec::with_capacity(uop.len());

    for i in 0..MAX_MULTIS {
        let path = format!("build/multicollection/{:06}.bin", i);
        let mut entry = match uop.get(&path) {
            Some(x) => x,
            None => continue,
        };

        let id = entry.read_u32::<Endian>()? as usize;
        if id != i {
            log::warn!("multi {i} has wrong ID {id}");
        }

        let count = entry.read_u32::<Endian>()? as usize;
        let mut components = Vec::with_capacity(count);

        for _ in 0..count {
            let graphic = entry.read_u16::<Endian>()?;
            let x = entry.read_i16::<Endian>()? as i32;
            let y = entry.read_i16::<Endian>()? as i32;
            let z = entry.read_i16::<Endian>()? as i32;
            let position = IVec3::new(x, y, z);
            let component_flags = entry.read_u16::<Endian>()?;
            let tooltip_count = entry.read_u32::<Endian>()? as usize;
            let mut tooltip_ids = Vec::with_capacity(tooltip_count);

            for _ in 0.. tooltip_count {
                let id = entry.read_u32::<Endian>()?;
                tooltip_ids.push(id);
            }

            components.push(MultiPrefabComponent {
                graphic,
                position,
                component_flags,
                tile_flags: TileFlags::empty(),
                tooltip_ids,
            });
        }

        prefabs.push(MultiPrefab {
            components,
        });
    }

    Ok(MultiData { prefabs })
}
