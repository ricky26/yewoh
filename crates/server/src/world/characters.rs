use std::sync::Arc;

use bevy::ecs::entity::{EntityHashMap, MapEntities, VisitEntities, VisitEntitiesMut};
use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::ecs::reflect::ReflectMapEntities;
use bevy::prelude::*;
use bevy::utils::{Entry, HashSet};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use yewoh::protocol::{AnyPacket, CharacterAnimation, CharacterEquipment, CharacterPredefinedAnimation, DeleteEntity, EntityFlags, EntityTooltipVersion, IntoAnyPacket, Race, UpdateCharacter, UpsertEntityCharacter, UpsertEntityStats, UpsertLocalPlayer};
use yewoh::{EntityId, Notoriety};
use yewoh::types::FixedString;

use crate::world::connection::{NetClient, OwningClient};
use crate::world::delta_grid::{delta_grid_cell, DeltaEntry, DeltaGrid, DeltaVersion};
use crate::world::entity::{Direction, Frozen, Hidden, Hue, MapPosition, RootPosition, Tooltip};
use crate::world::items::ValidItemPosition;
use crate::world::net_id::{OnDestroyNetEntity, NetId};
use crate::world::ServerSet;

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Deref, Component, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[serde(transparent)]
#[require(
    CharacterName,
    CharacterSex,
    CharacterRace,
    CharacterStats,
    CharacterSummary,
    Health,
    Stamina,
    Mana,
    Encumbrance,
    DamageResists,
    Frozen,
    Hidden,
    Protected,
    Invulnerable,
    Criminal,
    Murderer,
    Hue,
    Tooltip,
    MapPosition,
    RootPosition,
    Direction,
    WarMode,
    Allies,
    Enemies,
    Aggressors,
)]
pub struct CharacterBodyType(pub u16);

#[derive(Debug, Clone, Default, Deref, DerefMut, Component, Reflect)]
#[reflect(Component, Default)]
pub struct CharacterName(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq, Component, Reflect)]
#[reflect(Component, Default)]
pub enum CharacterSex {
    #[default]
    Male,
    Female,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect, Component)]
#[reflect(Component, Default)]
pub enum CharacterRace {
    #[default]
    Human,
    Elf,
    Gargoyle,
}

impl From<Race> for CharacterRace {
    fn from(value: Race) -> Self {
        match value {
            Race::Human => CharacterRace::Human,
            Race::Elf => CharacterRace::Elf,
            Race::Gargoyle => CharacterRace::Gargoyle,
        }
    }
}

impl From<CharacterRace> for Race {
    fn from(value: CharacterRace) -> Self {
        match value {
            CharacterRace::Human => Race::Human,
            CharacterRace::Elf => Race::Elf,
            CharacterRace::Gargoyle => Race::Gargoyle,
        }
    }
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component, Default)]
pub struct CharacterStats {
    pub str: u16,
    pub dex: u16,
    pub int: u16,
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component, Default)]
pub struct Health {
    pub hp: u16,
    pub max_hp: u16,
}

impl Default for Health {
    fn default() -> Self {
        Health {
            hp: 100,
            max_hp: 100,
        }
    }
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component, Default)]
pub struct Stamina {
    pub stamina: u16,
    pub max_stamina: u16,
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component, Default)]
pub struct Mana {
    pub mana: u16,
    pub max_mana: u16,
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component, Default)]
pub struct Encumbrance {
    pub encumbrance: u16,
    pub max_encumbrance: u16,
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component, Default)]
pub struct DamageResists {
    pub fire_resist: u16,
    pub cold_resist: u16,
    pub poison_resist: u16,
    pub energy_resist: u16,
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component, Default)]
pub struct CharacterSummary {
    pub gold: u32,
    pub armor: u16,
    pub stats_cap: u16,
    pub pet_count: u8,
    pub max_pets: u8,
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

#[derive(Clone, Copy, Debug, Default, Deref, DerefMut, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Protected(pub bool);

#[derive(Clone, Copy, Debug, Default, Deref, DerefMut, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Invulnerable(pub bool);

#[derive(Clone, Copy, Debug, Default, Deref, DerefMut, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Criminal(pub bool);

#[derive(Clone, Copy, Debug, Default, Deref, DerefMut, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Murderer(pub bool);

#[derive(Debug, Clone, Copy, Reflect, Component, VisitEntities, VisitEntitiesMut)]
#[reflect(Component, MapEntities)]
pub struct ControlledBy(pub Entity);

fn map_hash_set_entities(hash_set: &mut HashSet<Entity>, entity_mapper: &mut impl EntityMapper) {
    let old_entries = std::mem::take(hash_set);
    hash_set.reserve(old_entries.len());

    for entity in old_entries {
        let entity = entity_mapper.map_entity(entity);
        hash_set.insert(entity);
    }
}

#[derive(Debug, Clone, Default, Reflect, Component)]
#[reflect(Component, Default, MapEntities)]
pub struct Allies(pub HashSet<Entity>);

impl MapEntities for Allies {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        map_hash_set_entities(&mut self.0, entity_mapper);
    }
}

#[derive(Debug, Clone, Default, Reflect, Component)]
#[reflect(Component, Default, MapEntities)]
pub struct Enemies(pub HashSet<Entity>);

impl MapEntities for Enemies {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        map_hash_set_entities(&mut self.0, entity_mapper);
    }
}

#[derive(Debug, Clone, Default, Reflect, Component)]
#[reflect(Component, Default, MapEntities)]
pub struct Aggressors(pub HashSet<Entity>);

impl MapEntities for Aggressors {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        map_hash_set_entities(&mut self.0, entity_mapper);
    }
}

#[derive(Clone, Copy, Debug, Default, Deref, DerefMut, Reflect, Component)]
#[reflect(Default, Component)]
pub struct WarMode(pub bool);

#[derive(Debug, Clone, Event)]
pub struct OnCharacterAnimationStart {
    pub entity: Entity,
    pub location: MapPosition,
    pub animation: Animation,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientProfileRequest {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientProfileUpdateRequest {
    pub client_entity: Entity,
    pub target: Entity,
    pub new_profile: String,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientStatusRequest {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientSkillsRequest {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(QueryData)]
pub struct NotorietyQuery {
    pub protected: Ref<'static, Protected>,
    pub invulnerable: Ref<'static, Invulnerable>,
    pub criminal: Ref<'static, Criminal>,
    pub murderer: Ref<'static, Murderer>,
}

impl NotorietyQueryItem<'_> {
    pub fn is_changed(&self) -> bool {
        self.protected.is_changed() ||
            self.invulnerable.is_changed() ||
            self.criminal.is_changed() ||
            self.murderer.is_changed()
    }

    pub fn notoriety(&self) -> Notoriety {
        if **self.invulnerable {
            Notoriety::Invulnerable
        } else if **self.murderer {
            Notoriety::Murderer
        } else if **self.criminal {
            Notoriety::Criminal
        } else if **self.protected {
            Notoriety::Innocent
        } else {
            Notoriety::Neutral
        }
    }
}

#[derive(QueryData)]
pub struct CharacterQuery {
    pub body_type: Ref<'static, CharacterBodyType>,
    pub race: Ref<'static, CharacterRace>,
    pub sex: Ref<'static, CharacterSex>,
    pub hue: Ref<'static, Hue>,
    pub name: Ref<'static, CharacterName>,
    pub stats: Ref<'static, CharacterStats>,
    pub health: Ref<'static, Health>,
    pub mana: Ref<'static, Mana>,
    pub stamina: Ref<'static, Stamina>,
    pub encumbrance: Ref<'static, Encumbrance>,
    pub damage_resists: Ref<'static, DamageResists>,
    pub frozen: Ref<'static, Frozen>,
    pub hidden: Ref<'static, Hidden>,
    pub notoriety: NotorietyQuery,
    pub summary: Ref<'static, CharacterSummary>,
    pub war_mode: Ref<'static, WarMode>,
    pub tooltip: Ref<'static, Tooltip>,
    pub position: Ref<'static, MapPosition>,
    pub direction: Ref<'static, Direction>,
}

impl CharacterQueryItem<'_> {
    pub fn flags(&self) -> EntityFlags {
        let mut flags = EntityFlags::empty();

        if **self.frozen {
            flags |= EntityFlags::FROZEN;
        }

        if *self.sex == CharacterSex::Female {
            flags |= EntityFlags::FEMALE;
        }

        if **self.war_mode {
            flags |= EntityFlags::WAR_MODE;
        }

        if **self.hidden {
            flags |= EntityFlags::HIDDEN;
        }

        flags
    }

    pub fn notoriety(&self) -> Notoriety {
        self.notoriety.notoriety()
    }

    pub fn is_character_changed(&self) -> bool {
        self.body_type.is_changed() ||
            self.hue.is_changed() ||
            self.war_mode.is_changed() ||
            self.frozen.is_changed() ||
            self.hidden.is_changed() ||
            self.notoriety.is_changed()
    }

    pub fn is_status_changed(&self) -> bool {
        self.name.is_changed() ||
            self.health.is_changed()
    }

    pub fn is_stats_changed(&self) -> bool {
        self.name.is_changed() ||
            self.race.is_changed() ||
            self.sex.is_changed() ||
            self.stats.is_changed() ||
            self.health.is_changed() ||
            self.stamina.is_changed() ||
            self.mana.is_changed() ||
            self.encumbrance.is_changed() ||
            self.damage_resists.is_changed() ||
            self.summary.is_changed()
    }

    pub fn to_upsert(&self, id: EntityId, equipment: impl Into<SmallVec<[CharacterEquipment; 32]>>) -> UpsertEntityCharacter {
        UpsertEntityCharacter {
            id,
            body_type: **self.body_type,
            position: self.position.position,
            direction: (*self.direction).into(),
            hue: **self.hue,
            flags: self.flags(),
            notoriety: self.notoriety(),
            equipment: equipment.into(),
        }
    }

    pub fn to_update(&self, id: EntityId) -> UpdateCharacter {
        UpdateCharacter {
            id,
            body_type: **self.body_type,
            position: self.position.position,
            direction: (*self.direction).into(),
            hue: **self.hue,
            flags: self.flags(),
            notoriety: self.notoriety(),
        }
    }

    pub fn to_local_upsert(&self, id: EntityId) -> UpsertLocalPlayer {
        UpsertLocalPlayer {
            id,
            body_type: **self.body_type,
            hue: **self.hue,
            server_id: 0,
            flags: self.flags(),
            position: self.position.position,
            direction: (*self.direction).into(),
        }
    }

    pub fn to_status_packet(&self, id: EntityId) -> UpsertEntityStats {
        UpsertEntityStats {
            id,
            max_info_level: 0,
            name: FixedString::from_str(self.name.as_str()),
            allow_name_change: false,
            hp: self.health.hp,
            max_hp: self.health.max_hp,
            ..default()
        }
    }

    pub fn to_full_status_packet(&self, id: EntityId) -> UpsertEntityStats {
        UpsertEntityStats {
            id,
            max_info_level: 1,
            name: FixedString::from_str(self.name.as_str()),
            allow_name_change: true,
            female: *self.sex == CharacterSex::Female,
            race: (*self.race).into(),
            hp: self.health.hp,
            max_hp: self.health.max_hp,
            str: self.stats.str,
            dex: self.stats.dex,
            int: self.stats.int,
            stamina: self.stamina.stamina,
            max_stamina: self.stamina.max_stamina,
            mana: self.mana.mana,
            max_mana: self.mana.max_mana,
            gold: self.summary.gold,
            armor: self.summary.armor,
            weight: self.encumbrance.encumbrance,
            max_weight: self.encumbrance.max_encumbrance,
            stats_cap: self.summary.stats_cap,
            pet_count: self.summary.pet_count,
            max_pets: self.summary.max_pets,
            fire_resist: self.damage_resists.fire_resist,
            cold_resist: self.damage_resists.cold_resist,
            poison_resist: self.damage_resists.poison_resist,
            energy_resist: self.damage_resists.energy_resist,
            luck: self.summary.luck,
            damage_min: self.summary.damage_min,
            damage_max: self.summary.damage_max,
            tithing: self.summary.tithing,
            hit_chance_bonus: self.summary.hit_chance_bonus,
            swing_speed_bonus: self.summary.swing_speed_bonus,
            damage_chance_bonus: self.summary.damage_chance_bonus,
            reagent_cost_bonus: self.summary.reagent_cost_bonus,
            hp_regen: self.summary.hp_regen,
            stamina_regen: self.summary.stamina_regen,
            mana_regen: self.summary.mana_regen,
            damage_reflect: self.summary.damage_reflect,
            potion_bonus: self.summary.potion_bonus,
            defence_chance_bonus: self.summary.defence_chance_bonus,
            spell_damage_bonus: self.summary.spell_damage_bonus,
            cooldown_bonus: self.summary.cooldown_bonus,
            cast_time_bonus: self.summary.cast_time_bonus,
            mana_cost_bonus: self.summary.mana_cost_bonus,
            str_bonus: self.summary.str_bonus,
            dex_bonus: self.summary.dex_bonus,
            int_bonus: self.summary.int_bonus,
            hp_bonus: self.summary.hp_bonus,
            stamina_bonus: self.summary.stamina_bonus,
            mana_bonus: self.summary.mana_bonus,
            max_hp_bonus: self.summary.max_hp_bonus,
            max_stamina_bonus: self.summary.max_stamina_bonus,
            max_mana_bonus: self.summary.max_mana_bonus,
        }
    }
}

#[derive(QueryFilter)]
pub struct ChangedCharacterFilter {
    _query: Or<(
        Changed<CharacterBodyType>,
        Changed<Hue>,
        Changed<CharacterName>,
        Changed<CharacterSex>,
        Changed<Health>,
        Changed<Protected>,
        Changed<Invulnerable>,
        Changed<Criminal>,
        Changed<Murderer>,
        Changed<MapPosition>,
    )>,
}

#[derive(QueryFilter)]
pub struct ChangedFullStatusFilter {
    _query: Or<(
        Changed<CharacterName>,
        Changed<CharacterRace>,
        Changed<CharacterSex>,
        Changed<CharacterStats>,
        Changed<Health>,
        Changed<Stamina>,
        Changed<Mana>,
        Changed<Encumbrance>,
        Changed<DamageResists>,
        Changed<CharacterSummary>,
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
    mut removed_characters: EventReader<OnDestroyNetEntity>,
) {
    for (entity, net_id, character) in &characters_query {
        if net_id.is_changed() || character.is_character_changed() || character.position.is_changed() || character.direction.is_changed() {
            let update_packet = character.to_update(net_id.id).into_any_arc();
            let map_id = character.position.map_id;
            let position = character.position.position;
            let grid_cell = delta_grid_cell(position.truncate());
            let delta = delta_version.new_delta(DeltaEntry::CharacterChanged {
                entity,
                position: *character.position,
                update_packet,
            });

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

        if character.is_status_changed() {
            let position = *character.position;
            let grid_cell = delta_grid_cell(position.position.truncate());
            let packet = character.to_status_packet(net_id.id).into_any_arc();
            let delta = delta_version.new_delta(DeltaEntry::CharacterStatusChanged { entity, packet });
            if let Some(cell) = delta_grid.cell_at_mut(position.map_id, grid_cell) {
                cell.deltas.push(delta);
            }
        }

        if character.tooltip.is_changed() {
            let position = character.position;
            let grid_cell = delta_grid_cell(position.position.truncate());
            let packet = EntityTooltipVersion {
                id: net_id.id,
                revision: character.tooltip.version,
            }.into_any_arc();
            let delta = delta_version.new_delta(DeltaEntry::TooltipChanged { entity, packet });
            if let Some(cell) = delta_grid.cell_at_mut(position.map_id, grid_cell) {
                cell.deltas.push(delta);
            }
        }
    }

    for event in removed_characters.read() {
        let OnDestroyNetEntity { entity, id } = event.clone();
        if let Some(last_position) = cache.last_position.remove(&entity) {
            let grid_cell = delta_grid_cell(last_position.position.truncate());
            let packet = DeleteEntity {
                id,
            }.into_any_arc();
            let delta = delta_version.new_delta(DeltaEntry::CharacterRemoved { entity, packet });

            if let Some(cell) = delta_grid.cell_at_mut(last_position.map_id, grid_cell) {
                cell.deltas.push(delta);
            }
        }
    }
}

pub fn send_updated_full_status(
    clients: Query<&NetClient>,
    characters_query: Query<(&OwningClient, &NetId, CharacterQuery), ChangedFullStatusFilter>,
) {
   for (owner, net_id, character) in &characters_query {
       let Ok(client) = clients.get(owner.client_entity) else {
           continue;
       };

       let packet = character.to_full_status_packet(net_id.id);
       client.send_packet(packet);
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


pub fn queue_animations(
    delta_version: Res<DeltaVersion>,
    mut delta_grid: ResMut<DeltaGrid>,
    animation_targets: Query<(&NetId, &RootPosition)>,
    mut events: EventReader<OnCharacterAnimationStart>,
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
        .register_type::<CharacterBodyType>()
        .register_type::<CharacterName>()
        .register_type::<CharacterRace>()
        .register_type::<CharacterSex>()
        .register_type::<CharacterStats>()
        .register_type::<CharacterSummary>()
        .register_type::<Protected>()
        .register_type::<Invulnerable>()
        .register_type::<Criminal>()
        .register_type::<Murderer>()
        .register_type::<Allies>()
        .register_type::<Enemies>()
        .register_type::<Aggressors>()
        .register_type::<Health>()
        .register_type::<Stamina>()
        .register_type::<Mana>()
        .register_type::<Encumbrance>()
        .register_type::<DamageResists>()
        .register_type::<WarMode>()
        .register_type::<Animation>()
        .add_event::<OnCharacterAnimationStart>()
        .add_event::<OnClientProfileUpdateRequest>()
        .add_event::<OnClientProfileRequest>()
        .add_event::<OnClientSkillsRequest>()
        .add_event::<OnClientStatusRequest>()
        .add_systems(Last, (
            queue_animations.in_set(ServerSet::QueueDeltas),
            detect_character_changes.in_set(ServerSet::DetectChanges),
            send_updated_full_status.in_set(ServerSet::Send),
        ));
}
