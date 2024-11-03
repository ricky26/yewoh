use std::sync::Arc;

use bevy::ecs::entity::EntityHashMap;
use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::prelude::*;
use bevy::utils::Entry;
use yewoh::protocol::{AnyPacket, CharacterEquipment, DeleteEntity, EntityTooltipVersion, UpdateCharacter, UpsertEntityCharacter};
use yewoh::EntityId;

use crate::world::delta_grid::{delta_grid_cell, DeltaEntry, DeltaGrid, DeltaVersion};
use crate::world::entity::{BodyType, Flags, Hue, MapPosition, Notorious, Stats, Tooltip};
use crate::world::item::ValidItemPosition;
use crate::world::net_id::{NetId, RemovedNetIds};
use crate::world::ServerSet;

#[derive(QueryData)]
pub struct CharacterQuery {
    pub body_type: Ref<'static, BodyType>,
    pub hue: Ref<'static, Hue>,
    pub flags: Ref<'static, Flags>,
    pub notoriety: Ref<'static, Notorious>,
    pub tooltip: Ref<'static, Tooltip>,
    pub stats: Ref<'static, Stats>,
    pub position: Ref<'static, MapPosition>,
}

impl<'w> CharacterQueryItem<'w> {
    pub fn is_character_changed(&self) -> bool {
        self.body_type.is_changed() ||
            self.hue.is_changed() ||
            self.flags.is_changed() ||
            self.notoriety.is_changed()
    }

    pub fn to_upsert(&self, id: EntityId, equipment: Vec<CharacterEquipment>) -> UpsertEntityCharacter {
        UpsertEntityCharacter {
            id,
            body_type: **self.body_type,
            position: self.position.position,
            direction: self.position.direction,
            hue: **self.hue,
            flags: **self.flags,
            notoriety: **self.notoriety,
            equipment,
        }
    }

    pub fn to_update(&self, id: EntityId) -> UpdateCharacter {
        UpdateCharacter {
            id,
            body_type: **self.body_type,
            position: self.position.position,
            direction: self.position.direction,
            hue: **self.hue,
            flags: **self.flags,
            notoriety: **self.notoriety,
        }
    }
}

#[derive(QueryFilter)]
pub struct ChangedCharacterFilter {
    _query: Or<(
        Changed<BodyType>,
        Changed<Hue>,
        Changed<Flags>,
        Changed<Notorious>,
        Changed<Stats>,
        Changed<MapPosition>,
    )>,
}

#[derive(Default)]
pub struct CharacterCache {
    pub last_position: EntityHashMap<MapPosition>,
}

pub fn detect_character_changes(
    mut cache: Local<CharacterCache>,
    delta_version: Res<DeltaVersion>,
    mut delta_grid: ResMut<DeltaGrid>,
    characters_query: Query<
        (Entity, &NetId, CharacterQuery),
        (ValidItemPosition, Or<(Changed<NetId>, ChangedCharacterFilter)>),
    >,
    removed_characters: Res<RemovedNetIds>,
) {
    for (entity, net_id, character) in &characters_query {
        if character.is_character_changed() || character.position.is_changed() {
            let update_packet = Arc::new(AnyPacket::from_packet(character.to_update(net_id.id)));
            let map_id = character.position.map_id;
            let position = character.position.position;
            let grid_cell = delta_grid_cell(position.truncate());
            let delta = delta_version.new_delta(DeltaEntry::CharacterChanged { entity, update_packet });

            let mut position_entry = cache.last_position.entry(entity);
            if let Entry::Occupied(entry) = &mut position_entry {
                let last_position = entry.get();
                let last_grid_cell = delta_grid_cell(last_position.position.truncate());

                if last_position.map_id != map_id || grid_cell != last_grid_cell {
                    if let Some(cell) = delta_grid.cell_at_mut(last_position.map_id, last_position.position.truncate()) {
                        cell.deltas.push(delta.clone());
                    }
                }
            }

            if let Some(cell) = delta_grid.cell_at_mut(map_id, grid_cell) {
                cell.deltas.push(delta);
            }

            position_entry.insert(*character.position);
        }

        if character.tooltip.is_changed() {
            let position = character.position;
            let grid_cell = delta_grid_cell(position.position.truncate());
            let packet = AnyPacket::from_packet(EntityTooltipVersion {
                id: net_id.id,
                revision: character.tooltip.version,
            });
            let packet = Arc::new(packet);
            let delta = delta_version.new_delta(DeltaEntry::TooltipChanged { entity, packet });
            if let Some(cell) = delta_grid.cell_at_mut(position.map_id, grid_cell) {
                cell.deltas.push(delta);
            }
        }
    }

    for (entity, id) in removed_characters.removed_ids().iter().copied() {
        if let Some(last_position) = cache.last_position.remove(&entity) {
            let grid_cell = delta_grid_cell(last_position.position.truncate());
            let packet = Arc::new(AnyPacket::from_packet(DeleteEntity {
                id,
            }));
            let delta = delta_version.new_delta(DeltaEntry::CharacterRemoved { entity, packet });

            if let Some(cell) = delta_grid.cell_at_mut(last_position.map_id, grid_cell) {
                cell.deltas.push(delta);
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_systems(Last, (
            detect_character_changes.in_set(ServerSet::SendFirst),
        ));
}
