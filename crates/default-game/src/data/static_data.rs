use std::path::Path;
use tokio::fs;
use crate::data::cities::Cities;
use crate::data::maps::Maps;

#[derive(Debug, Clone)]
pub struct StaticData {
    pub cities: Cities,
    pub maps: Maps,
}

pub async fn load_from_directory(path: &Path) -> anyhow::Result<StaticData> {
    let cities = serde_yaml::from_slice(&fs::read(path.join("cities.yaml")).await?)?;
    let maps = serde_yaml::from_slice(&fs::read(path.join("maps.yaml")).await?)?;
    Ok(StaticData {
        cities,
        maps,
    })
}