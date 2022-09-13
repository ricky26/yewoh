use std::collections::HashMap;
use std::path::Path;
use futures::future;

use glam::UVec2;
use serde::{Deserialize, Serialize};
use tokio::fs;
use yewoh::assets::map::load_map;

use yewoh_server::world::net::{MapInfo, MapInfos};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Map {
    pub name: String,
    pub size: UVec2,
    pub season: u8,
    pub no_assets: bool,

    pub tile_ids: Vec<u16>,
    pub heights: Vec<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Maps {
    pub maps: HashMap<u8, Map>,
}

impl Maps {
    pub async fn load(info_path: &Path, uo_data_path: &Path) -> anyhow::Result<Maps> {
        let mut maps = serde_yaml::from_slice::<Maps>(&fs::read(info_path).await?)?;

        future::try_join_all(maps.maps.iter_mut()
            .map(|(id, map)| async move {
                if !map.no_assets {
                    log::info!("Loading map {id}...");
                    let data = load_map(uo_data_path, *id as usize, map.size.x as usize, map.size.y as usize).await?;
                    map.tile_ids = data.tile_ids;
                    map.heights = data.heights;
                }
                Ok::<_, anyhow::Error>(())
            })).await?;

        Ok(maps)
    }

    pub fn map_infos(&self) -> MapInfos {
        let mut maps = HashMap::with_capacity(self.maps.len());
        for (key, map) in self.maps.iter() {
            maps.insert(*key, MapInfo {
                size: map.size,
                season: map.season,
            });
        }

        MapInfos {
            maps,
        }
    }
}
