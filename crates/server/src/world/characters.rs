use std::sync::Arc;

use bevy::ecs::entity::EntityHashMap;
use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::prelude::*;
use bevy::utils::Entry;
use serde::{Deserialize, Serialize};
use yewoh::protocol::{AnyPacket, CharacterAnimation, CharacterEquipment, CharacterPredefinedAnimation, DeleteEntity, EntityTooltipVersion, Race, UpdateCharacter, UpsertEntityCharacter, UpsertEntityStats};
use yewoh::types::FixedString;
use yewoh::EntityId;

use crate::world::delta_grid::{delta_grid_cell, DeltaEntry, DeltaGrid, DeltaVersion};
use crate::world::entity::{BodyType, Flags, Hue, MapPosition, Notorious, RootPosition, Tooltip};
use crate::world::items::ValidItemPosition;
use crate::world::net_id::{NetEntityDestroyed, NetId};
use crate::world::ServerSet;

#[derive(Debug, Clone, Default, Component, Reflect, Eq, PartialEq, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct Stats {
    pub name: String,
    pub female: bool,
    #[reflect(ignore)]
    pub race: Race,
    pub hp: u16,
    pub max_hp: u16,
    pub str: u16,
    pub dex: u16,
    pub int: u16,
    pub stamina: u16,
    pub max_stamina: u16,
    pub mana: u16,
    pub max_mana: u16,
    pub gold: u32,
    pub armor: u16,
    pub weight: u16,
    pub max_weight: u16,
    pub stats_cap: u16,
    pub pet_count: u8,
    pub max_pets: u8,
    pub fire_resist: u16,
    pub cold_resist: u16,
    pub poison_resist: u16,
    pub energy_resist: u16,
    pub luck: u16,
    pub damage_min: u16,
    pub damage_max: u16,
    pub tithing: u32,
    pub hit_chance_bonus: u16,
    pub swing_speed_bonus: u16,
    pub damage_chance_bonus: u16,
    pub reagent_cost_bonus: u16,
    pub hp_regen: u16,
    pub stamina_regen: u16,
    pub mana_regen: u16,
    pub damage_reflect: u16,
    pub potion_bonus: u16,
    pub defence_chance_bonus: u16,
    pub spell_damage_bonus: u16,
    pub cooldown_bonus: u16,
    pub cast_time_bonus: u16,
    pub mana_cost_bonus: u16,
    pub str_bonus: u16,
    pub dex_bonus: u16,
    pub int_bonus: u16,
    pub hp_bonus: u16,
    pub stamina_bonus: u16,
    pub mana_bonus: u16,
    pub max_hp_bonus: u16,
    pub max_stamina_bonus: u16,
    pub max_mana_bonus: u16,
}

impl Stats {
    pub fn upsert(&self, id: EntityId, owned: bool) -> UpsertEntityStats {
        let max_info_level = if owned { 8 } else { 0 };
        UpsertEntityStats {
            id,
            max_info_level,
            name: FixedString::from_str(&self.name),
            allow_name_change: owned,
            female: self.female,
            race: self.race,
            hp: self.hp,
            max_hp: self.max_hp,
            str: self.str,
            dex: self.dex,
            int: self.int,
            stamina: self.stamina,
            max_stamina: self.max_stamina,
            mana: self.mana,
            max_mana: self.max_mana,
            gold: self.gold,
            armor: self.armor,
            weight: self.weight,
            max_weight: self.max_weight,
            stats_cap: self.stats_cap,
            pet_count: self.pet_count,
            max_pets: self.max_pets,
            fire_resist: self.fire_resist,
            cold_resist: self.cold_resist,
            poison_resist: self.poison_resist,
            energy_resist: self.energy_resist,
            luck: self.luck,
            damage_min: self.damage_min,
            damage_max: self.damage_max,
            tithing: self.tithing,
            hit_chance_bonus: self.hit_chance_bonus,
            swing_speed_bonus: self.swing_speed_bonus,
            damage_chance_bonus: self.damage_chance_bonus,
            reagent_cost_bonus: self.reagent_cost_bonus,
            hp_regen: self.hp_regen,
            stamina_regen: self.stamina_regen,
            mana_regen: self.mana_regen,
            damage_reflect: self.damage_reflect,
            potion_bonus: self.potion_bonus,
            defence_chance_bonus: self.defence_chance_bonus,
            spell_damage_bonus: self.spell_damage_bonus,
            cooldown_bonus: self.cooldown_bonus,
            cast_time_bonus: self.cast_time_bonus,
            mana_cost_bonus: self.mana_cost_bonus,
            str_bonus: self.str_bonus,
            dex_bonus: self.dex_bonus,
            int_bonus: self.int_bonus,
            hp_bonus: self.hp_bonus,
            stamina_bonus: self.stamina_bonus,
            mana_bonus: self.mana_bonus,
            max_hp_bonus: self.max_hp_bonus,
            max_stamina_bonus: self.max_stamina_bonus,
            max_mana_bonus: self.max_mana_bonus,
        }
    }
}

#[derive(Debug, Clone, Event)]
pub struct AnimationStartedEvent {
    pub entity: Entity,
    pub location: MapPosition,
    pub animation: Animation,
}

#[derive(Debug, Clone, Event)]
pub struct ProfileEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub new_profile: Option<String>,
}

#[derive(Debug, Clone, Event)]
pub struct RequestSkillsEvent {
    pub client_entity: Entity,
    pub target: Entity,
}

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
        (Entity, Ref<NetId>, CharacterQuery),
        (ValidItemPosition, Or<(Changed<NetId>, ChangedCharacterFilter)>),
    >,
    mut removed_characters: EventReader<NetEntityDestroyed>,
) {
    for (entity, net_id, character) in &characters_query {
        if net_id.is_changed() || character.is_character_changed() || character.position.is_changed() {
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

    for event in removed_characters.read() {
        let NetEntityDestroyed { entity, id } = event.clone();
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

#[derive(Debug, Clone, Default, Reflect, Deserialize)]
#[reflect(Default, Deserialize)]
#[serde(default)]
pub struct AnimationSlice {
    pub animation_id: u16,
    pub frame_count: u16,
    pub repeat_count: u16,
    pub reverse: bool,
    pub speed: u8,
}

impl AnimationSlice {
    pub fn to_packet(&self, target_id: EntityId) -> CharacterAnimation {
        CharacterAnimation {
            target_id,
            animation_id: self.animation_id,
            frame_count: self.frame_count,
            repeat_count: self.repeat_count,
            reverse: self.reverse,
            speed: self.speed,
        }
    }
}

#[derive(Debug, Clone, Default, Reflect, Deserialize)]
#[reflect(Default, Deserialize)]
#[serde(default)]
pub struct PredefinedAnimation {
    pub kind: u16,
    pub action: u16,
    pub variant: u8,
}

impl PredefinedAnimation {
    pub fn to_packet(&self, target_id: EntityId) -> CharacterPredefinedAnimation {
        CharacterPredefinedAnimation {
            target_id,
            kind: self.kind,
            action: self.action,
            variant: self.variant,
        }
    }
}

#[derive(Debug, Clone, Reflect, Deserialize)]
#[reflect(Default, Deserialize)]
#[serde(untagged)]
pub enum Animation {
    Slice(AnimationSlice),
    Predefined(PredefinedAnimation),
}

impl Default for Animation {
    fn default() -> Self {
        Animation::Slice(Default::default())
    }
}

impl Animation {
    pub fn to_packet(&self, target_id: EntityId) -> AnyPacket {
        match self {
            Animation::Slice(anim) => anim.to_packet(target_id).into(),
            Animation::Predefined(anim) => anim.to_packet(target_id).into(),
        }
    }
}


pub fn detect_animations(
    delta_version: Res<DeltaVersion>,
    mut delta_grid: ResMut<DeltaGrid>,
    animation_targets: Query<(&NetId, &RootPosition)>,
    mut events: EventReader<AnimationStartedEvent>,
) {
    for event in events.read() {
        let Ok((net_id, position)) = animation_targets.get(event.entity) else {
            warn!("Got animation for {} which is not animatable.", event.entity);
            continue;
        };
        
        let grid_cell = delta_grid_cell(position.position.truncate());
        let packet = Arc::new(event.animation.to_packet(net_id.id));
        
        if let Some(cell) = delta_grid.cell_at_mut(position.map_id, grid_cell) {
            cell.deltas.push(delta_version.new_delta(DeltaEntry::CharacterAnimation {
                entity: event.entity,
                packet,
            }));
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<Animation>()
        .register_type::<Stats>()
        .add_event::<AnimationStartedEvent>()
        .add_event::<ProfileEvent>()
        .add_event::<RequestSkillsEvent>()
        .add_systems(Last, (
            (
                detect_character_changes,
                detect_animations,
            ).in_set(ServerSet::DetectChanges),
        ));
}
