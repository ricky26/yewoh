use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use bevy::ecs::entity::MapEntities;
use bevy::ecs::reflect::ReflectMapEntities;
use bevy::prelude::*;
use glam::{IVec2, IVec3};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use yewoh::protocol::{EntityFlags, EquipmentSlot, Race, UpsertEntityStats};
use yewoh::types::FixedString;
use yewoh::{Direction, EntityId, Notoriety};

use crate::math::IVecExt;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Reflect, Component)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Flags {
    #[reflect(remote = crate::remote_reflect::EntityFlagsRemote)]
    pub flags: EntityFlags,
}

impl Serialize for Flags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.flags.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Flags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        Ok(Flags {
            flags: EntityFlags::deserialize(deserializer)?,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Reflect, Component, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Notorious(#[reflect(remote = crate::remote_reflect::NotorietyRemote)] pub Notoriety);

impl Deref for Notorious {
    type Target = Notoriety;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for Notorious {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

#[derive(Debug, Clone, Eq, PartialEq, Reflect)]
pub struct CharacterEquipped {
    pub entity: Entity,
    #[reflect(remote = crate::remote_reflect::EquipmentSlotRemote)]
    pub slot: EquipmentSlot,
}

impl CharacterEquipped {
    pub fn new(slot: EquipmentSlot, entity: Entity) -> CharacterEquipped {
        Self {
            entity,
            slot,
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq, Component, Reflect)]
#[reflect(Component, MapEntities)]
pub struct Character {
    pub body_type: u16,
    pub hue: u16,
    pub equipment: Vec<CharacterEquipped>,
}

impl MapEntities for Character {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        for equipment in &mut self.equipment {
            equipment.entity = entity_mapper.map_entity(equipment.entity);
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Component, Reflect)]
#[reflect(Component, MapEntities)]
pub struct EquippedBy {
    pub parent: Entity,
    #[reflect(remote = crate::remote_reflect::EquipmentSlotRemote)]
    pub slot: EquipmentSlot,
}

impl FromWorld for EquippedBy {
    fn from_world(_world: &mut World) -> Self {
        EquippedBy { parent: Entity::PLACEHOLDER, slot: EquipmentSlot::default() }
    }
}

impl MapEntities for EquippedBy {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.parent = entity_mapper.map_entity(self.parent);
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Component, Reflect)]
#[reflect(Component)]
pub struct Quantity {
    pub quantity: u16,
}

impl Default for Quantity {
    fn default() -> Self {
        Quantity { quantity: 1 }
    }
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Component, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Graphic {
    pub id: u16,
    pub hue: u16,
}

#[derive(Debug, Default, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
pub struct Multi {
    pub id: u16,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Component, Reflect, Serialize, Deserialize)]
#[reflect(Default, Component)]
pub struct Location {
    pub position: IVec3,
    pub map_id: u8,
    #[serde(default)]
    #[reflect(remote = crate::remote_reflect::DirectionRemote)]
    pub direction: Direction,
}

impl Location {
    pub fn manhattan_distance(&self, other: &Location) -> Option<i32> {
        if self.map_id == other.map_id {
            Some(self.position.truncate().manhattan_distance(&other.position.truncate()))
        } else {
            None
        }
    }

    pub fn in_range(&self, other: &Location, range: i32) -> bool {
        self.manhattan_distance(other).map_or(false, |distance| distance <= range)
    }
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component, MapEntities)]
pub struct Container {
    pub gump_id: u16,
    pub items: Vec<Entity>,
}

impl MapEntities for Container {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        for item in &mut self.items {
            *item = entity_mapper.map_entity(*item);
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Component, Reflect)]
#[reflect(Component, MapEntities)]
pub struct ParentContainer {
    pub parent: Entity,
    pub position: IVec2,
    pub grid_index: u8,
}

impl FromWorld for ParentContainer {
    fn from_world(_world: &mut World) -> Self {
        ParentContainer {
            parent: Entity::PLACEHOLDER,
            position: IVec2::ZERO,
            grid_index: 0,
        }
    }
}

impl MapEntities for ParentContainer {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.parent = entity_mapper.map_entity(self.parent);
    }
}

#[derive(Debug, Clone, Default, Component, Reflect, Eq, PartialEq, Serialize, Deserialize)]
#[reflect(Component)]
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

#[derive(Debug, Clone, Eq, PartialEq, Reflect)]
pub struct TooltipLine {
    pub text_id: u32,
    pub arguments: String,
    pub priority: u32,
}

impl TooltipLine {
    pub fn from_static(text_id: u32, priority: u32) -> TooltipLine {
        Self {
            text_id,
            arguments: Default::default(),
            priority,
        }
    }

    pub fn from_str(text: String, priority: u32) -> TooltipLine {
        Self {
            text_id: 1042971,
            arguments: text,
            priority,
        }
    }
}

impl PartialOrd for TooltipLine {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.text_id.cmp(&other.text_id))
    }
}

impl Ord for TooltipLine {
    fn cmp(&self, other: &Self) -> Ordering {
        self.text_id.cmp(&other.text_id)
    }
}

#[derive(Debug, Clone, Default, Component, Eq, PartialEq, Reflect)]
#[reflect(Component)]
pub struct Tooltip {
    pub entries: HashMap<String, TooltipLine>,
}

impl Tooltip {
    pub fn contains(&self, key: &str, line: &TooltipLine) -> bool {
        self.entries.get(key).map_or(false, |e| e == line)
    }

    pub fn push(&mut self, key: impl Into<String>, line: TooltipLine) {
        self.entries.insert(key.into(), line);
    }

    pub fn push_mut(this: &mut Mut<Tooltip>, key: impl AsRef<str> + ToOwned<Owned = String>, line: TooltipLine) {
        if !this.contains(key.as_ref(), &line) {
            this.push(key.to_owned(), line);
        }
    }
}

#[derive(Debug, Clone, Component, Eq, PartialEq, Reflect)]
#[reflect(Component, MapEntities)]
pub struct AttackTarget {
    pub target: Entity,
}

impl FromWorld for AttackTarget {
    fn from_world(_world: &mut World) -> Self {
        AttackTarget {
            target: Entity::PLACEHOLDER,
        }
    }
}

impl MapEntities for AttackTarget {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.target = entity_mapper.map_entity(self.target);
    }
}
