use std::path::{Path, PathBuf};
use bevy::prelude::*;
use tokio::fs;

use crate::data::cities::Cities;
use crate::data::locations::Locations;
use crate::data::maps::Maps;
use crate::data::skills::Skills;

#[derive(Debug, Clone, Reflect, Resource)]
#[reflect(Resource)]
pub struct DataPath(pub PathBuf);

#[derive(Debug, Clone, Resource)]
pub struct StaticData {
    pub cities: Cities,
    pub maps: Maps,
    pub skills: Skills,
    pub locations: Locations,
}

pub async fn load_from_directory(data_path: &Path) -> anyhow::Result<StaticData> {
    let cities = serde_yaml::from_slice(&fs::read(data_path.join("cities.yaml")).await?)?;
    let maps = serde_yaml::from_slice(&fs::read(data_path.join("maps.yaml")).await?)?;
    let skills = serde_yaml::from_slice(&fs::read(data_path.join("skills.yaml")).await?)?;
    let mut locations = serde_yaml::from_slice::<Locations>(&fs::read(data_path.join("locations.yaml")).await?)?;
    locations.add_cities(&cities);
    locations.sort();
    Ok(StaticData {
        cities,
        maps,
        skills,
        locations,
    })
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<DataPath>();
}
