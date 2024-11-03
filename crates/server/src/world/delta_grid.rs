use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use bevy::prelude::*;
use bevy::utils::HashMap;
use yewoh::protocol::AnyPacket;

use crate::world::map::MapInfos;
use crate::world::ServerSet;

const DELTA_CELL_SIZE: usize = 16;

fn cell_index(width: usize, len: usize, position: IVec2) -> Option<usize> {
    if position.x < 0 ||
        position.x as usize >= width ||
        position.y < 0 {
        None
    } else {
        let index = (position.x as usize) + (position.y as usize) * width;
        if index >= len {
            None
        } else {
            Some(index)
        }
    }
}

pub fn delta_grid_cell(position: IVec2) -> IVec2 {
    (position + (DELTA_CELL_SIZE as i32 - 1)) / (DELTA_CELL_SIZE as i32)
}

#[derive(Clone, Debug)]
pub enum DeltaEntry {
    ItemChanged { entity: Entity, parent: Option<Entity>, packet: Arc<AnyPacket> },
    ItemRemoved { entity: Entity, packet: Arc<AnyPacket> },
    CharacterChanged { entity: Entity, update_packet: Arc<AnyPacket> },
    CharacterRemoved { entity: Entity, packet: Arc<AnyPacket> },
    CharacterAnimation { entity: Entity, packet: Arc<AnyPacket> },
    CharacterDamaged { entity: Entity, packet: Arc<AnyPacket> },
    CharacterSwing { entity: Entity, target: Entity, packet: Arc<AnyPacket> },
    TooltipChanged { entity: Entity, packet: Arc<AnyPacket> },
}

#[derive(Clone, Debug)]
pub struct Delta {
    pub version: u32,
    pub entry: DeltaEntry,
}

impl Delta {
    pub fn new(version: u32, entry: DeltaEntry) -> Delta {
        Delta {
            version,
            entry,
        }
    }
}

#[derive(Clone, Debug, Default, Reflect)]
pub struct DeltaCell {
    #[reflect(ignore)]
    pub deltas: Vec<Delta>,
}

#[derive(Clone, Debug, Default, Reflect)]
pub struct MapDeltaGrid {
    width: usize,
    cells: Vec<DeltaCell>,
}

impl MapDeltaGrid {
    fn cell_index(&self, position: IVec2) -> Option<usize> {
        cell_index(self.width, self.cells.len(), position)
    }

    pub fn new(width: usize, height: usize) -> MapDeltaGrid {
        let width = width.div_ceil(DELTA_CELL_SIZE);
        let height = height.div_ceil(DELTA_CELL_SIZE);
        let num = width * height;
        let mut cells = Vec::new();
        cells.resize_with(num, || DeltaCell::default());
        MapDeltaGrid {
            width,
            cells,
        }
    }

    pub fn cell_at(&self, position: IVec2) -> Option<&DeltaCell> {
        let index = self.cell_index(position)?;
        Some(&self.cells[index])
    }

    pub fn cell_at_mut(&mut self, position: IVec2) -> Option<&mut DeltaCell> {
        let index = self.cell_index(position)?;
        Some(&mut self.cells[index])
    }
}

#[derive(Clone, Debug, Default, Reflect, Resource)]
#[reflect(Resource)]
pub struct DeltaGrid {
    pub maps: HashMap<u8, MapDeltaGrid>,
}

impl DeltaGrid {
    pub fn new(map_infos: &MapInfos) -> DeltaGrid {
        let mut maps = HashMap::new();

        for (map_id, map) in &map_infos.maps {
            maps.insert(*map_id, MapDeltaGrid::new(map.size.x as usize, map.size.y as usize));
        }

        DeltaGrid {
            maps,
        }
    }

    pub fn cell_at(&self, map_id: u8, position: IVec2) -> Option<&DeltaCell> {
        let map = self.maps.get(&map_id)?;
        map.cell_at(position)
    }

    pub fn cell_at_mut(&mut self, map_id: u8, position: IVec2) -> Option<&mut DeltaCell> {
        let map = self.maps.get_mut(&map_id)?;
        map.cell_at_mut(position)
    }
}

pub fn reset_delta_grid(mut grid: ResMut<DeltaGrid>) {
    for map in grid.maps.values_mut() {
        for cell in &mut map.cells {
            cell.deltas.clear();
        }
    }
}

#[derive(Debug, Default, Reflect, Resource)]
#[reflect(Resource)]
pub struct DeltaVersion(AtomicU32);

impl DeltaVersion {
    pub fn reset(&self) {
        self.0.store(0, Ordering::Relaxed);
    }

    pub fn allocate(&self) -> u32 {
        self.0.fetch_add(1, Ordering::Relaxed)
    }

    pub fn new_delta(&self, entry: DeltaEntry) -> Delta {
        Delta::new(self.allocate(), entry)
    }
}

pub fn reset_delta_version(version: Res<DeltaVersion>) {
    version.reset();
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<DeltaGrid>()
        .register_type::<DeltaVersion>()
        .init_resource::<DeltaGrid>()
        .init_resource::<DeltaVersion>()
        .add_systems(Update, (
            (
                reset_delta_version,
                reset_delta_grid,
            ).in_set(ServerSet::SendLast),
        ));
}
