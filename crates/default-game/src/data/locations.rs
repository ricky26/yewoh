use glam::IVec3;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::data::cities::Cities;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Location {
    pub map_id: u32,
    pub position: IVec3,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Locations {
    pub locations: IndexMap<String, Location>,
}

impl Locations {
    pub fn add_cities(&mut self, cities: &Cities) {
        for city in &cities.cities {
            let key = format!("Cities/{}", city.name);
            let location = Location {
                map_id: city.map_id,
                position: city.position,
            };
            self.locations.insert(key, location);
        }
    }

    pub fn sort(&mut self) {
        self.locations.sort_keys();
    }

    pub fn iter_level<'a>(&'a self, prefix: &'a str) -> LocationLevelIter<'a> {
        let offset = self.locations.binary_search_by_key(&prefix, |k, _| k.as_ref())
            .unwrap_or_else(|e| e);
        LocationLevelIter {
            locations: self,
            prefix,
            offset,
        }
    }
}

#[derive(Clone, Debug)]
pub enum LocationLevelAction<'a> {
    Descend(&'a str),
    Location(&'a Location),
}

#[derive(Clone, Debug)]
pub struct LocationLevelIter<'a> {
    locations: &'a Locations,
    prefix: &'a str,
    offset: usize,
}

impl<'a> Iterator for LocationLevelIter<'a> {
    type Item = (&'a str, LocationLevelAction<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let (next_key, next_value) = &self.locations.locations.get_index(self.offset)?;
        let rest = next_key.strip_prefix(self.prefix)?;

        self.offset += 1;
        if let Some(slash) = rest.find('/') {
            let result = &rest[..slash];
            let new_prefix = &next_key.as_str()[..(self.prefix.len() + slash + 1)];

            while self.offset < self.locations.locations.len() {
                let Some((follow_key, _)) = &self.locations.locations.get_index(self.offset) else {
                    break;
                };

                if !follow_key.starts_with(new_prefix) {
                    break;
                }

                self.offset += 1;
            }

            Some((result, LocationLevelAction::Descend(new_prefix)))
        } else {
            Some((rest, LocationLevelAction::Location(next_value)))
        }
    }
}
