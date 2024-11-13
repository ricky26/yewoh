use bevy::prelude::*;
use glam::IVec3;
use serde::{Deserialize, Serialize};

use yewoh::protocol::StartingCity;

#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
#[serde(default)]
pub struct City {
    pub name: String,
    pub building: String,
    pub map_id: u32,
    pub description_id: u32,
    pub position: IVec3,
}

#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
pub struct Cities {
    pub cities: Vec<City>,
}

impl Cities {
    pub fn to_starting_cities(&self) -> Vec<StartingCity> {
        self.cities.iter()
            .enumerate()
            .map(|(index, city)| StartingCity {
                index: index as u8,
                city: city.name.clone(),
                building: city.building.clone(),
                position: city.position,
                map_id: city.map_id,
                description_id: city.description_id,
            })
            .collect()
    }
}
