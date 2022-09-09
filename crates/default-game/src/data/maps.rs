use std::collections::HashMap;

use glam::UVec2;
use serde::{Deserialize, Serialize};

use yewoh_server::world::client::{MapInfo, MapInfos};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Map {
    id: u8,
    name: String,
    size: UVec2,
    season: u8,
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
            });
        }

        MapInfos {
            maps,
        }
    }
}
