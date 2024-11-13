use std::path::Path;

use anyhow::Context;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::utils::HashMap;
use clap::Parser;
use serde::Deserialize;
use yewoh::assets::tiles::{TileData, TileFlags};
use yewoh_server::world::entity::{DirectionMask, MapPosition};
use yewoh_server::world::map::TileDataResource;
use yewoh_server::world::spatial::{Area2Iter, ItemEntry, SpatialQuery};

use crate::commands::{TextCommand, TextCommandQueue, TextCommandRegistrationExt};
use crate::data::prefabs::PrefabLibraryWorldExt;
use crate::data::static_data::{DataPath, StaticData};
use crate::items::buildings::doors::DoorCcw;

const CONFIG_PATH: &'static str = "spawn_doors.yaml";

#[derive(Clone, Debug, Deserialize)]
struct DoorArea {
    pub min: IVec2,
    pub max: IVec2,
}

#[derive(Clone, Debug, Deserialize)]
struct DoorRegion {
    pub maps: Vec<u8>,
    pub areas: Vec<DoorArea>,
}

#[derive(Clone, Debug, Deserialize)]
struct DoorFrame {
    pub graphic: u16,
    pub directions: DirectionMask,
}

#[derive(Clone, Debug, Deserialize)]
struct Config {
    pub regions: Vec<DoorRegion>,
    pub frames: Vec<DoorFrame>,
}

fn load_config(path: &Path) -> anyhow::Result<Config> {
    let contents = std::fs::read_to_string(path)
        .context("reading config file")?;
    let config = serde_yaml::from_str(&contents)
        .context("parsing config")?;
    Ok(config)
}

pub fn spawn_doors(
    mut args: In<SpawnDoors>,
    mut commands: Commands,
    data_path: Res<DataPath>,
    static_data: Res<StaticData>,
    tile_data: Res<TileDataResource>,
    spatial_query: SpatialQuery,
) {
    let tile_data = &tile_data.tile_data;
    let config = match load_config(&data_path.0.join(CONFIG_PATH)) {
        Ok(x) => x,
        Err(err) => {
            warn!("failed to load spawn doors config: {err}");
            return;
        }
    };

    if args.maps.is_empty() {
        args.maps.extend(static_data.maps.maps.keys().copied());
    }

    let frames = HashMap::from_iter(config.frames.into_iter()
        .map(|entry| (entry.graphic, entry.directions)));

    for map in args.maps.drain(..) {
        let Some(static_items) = spatial_query.static_items.lookup.maps.get(&map) else {
            continue;
        };

        let areas = config.regions.iter()
            .filter(|r| r.maps.contains(&map))
            .flat_map(|r| r.areas.iter());
        for area in areas {
            let min = area.min;
            let max = area.max;
            for p in Area2Iter::new(min, max) {
                for entry_a in static_items.entries_at(p) {
                    let Some(directions) = frames.get(&entry_a.graphic) else {
                        continue;
                    };

                    // Only need to check S/E because the other half of the door-frame will
                    // effectively check N/W.
                    let to_test = *directions & (DirectionMask::SOUTH | DirectionMask::EAST);
                    for direction in to_test.iter_directions() {
                        let opposite_mask = DirectionMask::from(direction.opposite());
                        let delta = direction.as_vec2();
                        let is_hit = |tile_data: &TileData, a: &ItemEntry, b: &ItemEntry| {
                            if (b.z_min < a.z_min) || (b.z_min >= a.z_max) {
                                return false;
                            }

                            let Some(data) = tile_data.items.get(b.graphic as usize) else {
                                return true;
                            };

                            data.flags.contains(TileFlags::IMPASSABLE)
                        };
                        let is_frame = move |frames: &HashMap<u16, DirectionMask>, z, entry: &ItemEntry| {
                            if entry.z_min != z {
                                return false;
                            }

                            let Some(frame) = frames.get(&entry.graphic) else {
                                return false;
                            };

                            frame.contains(opposite_mask)
                        };

                        let p1 = p + delta;
                        let p1_items = static_items.entries_at(p1);
                        if p1_items.iter().any(|entry_b| is_hit(tile_data, entry_a, entry_b)) {
                            continue;
                        }

                        let p2 = p1 + delta;
                        let p2_items = static_items.entries_at(p2);
                        if p2_items.iter().any(|entry_b| is_frame(&frames, entry_a.z_min, entry_b)) {
                            commands.fabricate_prefab("door")
                                .insert((
                                    MapPosition {
                                        position: p1.extend(entry_a.z_min),
                                        map_id: map,
                                    },
                                    direction.rotate(2),
                                ));
                            continue;
                        }

                        if p2_items.iter().any(|entry_b| is_hit(tile_data, entry_a, entry_b)) {
                            continue;
                        }

                        let p3 = p2 + delta;
                        let p3_items = static_items.entries_at(p3);
                        if p3_items.iter().any(|entry_b| is_frame(&frames, entry_a.z_min, entry_b)) {
                            commands.fabricate_prefab("door")
                                .insert((
                                    MapPosition {
                                        position: p1.extend(entry_a.z_min),
                                        map_id: map,
                                    },
                                    direction.rotate(2),
                                ));
                            commands.fabricate_prefab("door")
                                .insert((
                                    MapPosition {
                                        position: p2.extend(entry_a.z_min),
                                        map_id: map,
                                    },
                                    direction.rotate(6),
                                    DoorCcw,
                                ));
                        }
                    }
                }
            }
        }
    }
}

#[derive(Parser, Resource)]
pub struct SpawnDoors {
    #[arg(long)]
    pub maps: Vec<u8>,
}

impl TextCommand for SpawnDoors {
    fn aliases() -> &'static [&'static str] {
        &["spawn-doors"]
    }
}

pub fn trigger_spawn_doors(
    mut exec: TextCommandQueue<SpawnDoors>,
    mut commands: Commands,
) {
    for (_, cmd) in exec.iter() {
        commands.queue(|world: &mut World| {
            if let Err(err) = world.run_system_once_with(cmd, spawn_doors) {
                warn!("failed to spawn doors: {err}");
            }
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_text_command::<SpawnDoors>()
        .add_systems(Update, (
            trigger_spawn_doors,
        ));
}
