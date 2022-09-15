use std::collections::HashMap;

use glam::UVec2;
use serde::{Deserialize, Serialize};

use yewoh_server::world::net::{MapInfo, MapInfos};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Map {
    pub name: String,
    pub size: UVec2,
    pub season: u8,
    pub no_assets: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Maps {
    pub maps: HashMap<u8, Map>,
}

impl Maps {
    pub fn map_infos(&self) -> MapInfos {
        let mut maps = HashMap::with_capacity(self.maps.len());
        for (key, map) in self.maps.iter() {
            maps.insert(*key, MapInfo {
                size: map.size,
                season: map.season,
                is_virtual: map.no_assets,
            });
        }

        MapInfos {
            maps,
        }
    }
}
