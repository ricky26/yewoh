use bevy_ecs::prelude::*;
use glam::UVec3;

use yewoh::{Direction, EntityId};

#[derive(Debug, Clone, Copy, Component)]
pub struct NetEntity {
    pub id: EntityId,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct NetOwner {
    pub connection: Entity,
}

#[derive(Debug, Clone, Copy, Component)]
pub enum EntityVisualKind {
    Graphic(u16),
    Body(u16),
    Multi(u16),
}

impl Default for EntityVisualKind {
    fn default() -> Self {
        EntityVisualKind::Graphic(0)
    }
}

#[derive(Debug, Clone, Copy, Component, Default)]
pub struct EntityVisual {
    pub kind: EntityVisualKind,
    pub hue: u16,
}

#[derive(Debug, Clone, Copy, Component, Default)]
pub struct MapPosition {
    pub position: UVec3,
    pub map_id: u8,
    pub direction: Direction,
}

#[derive(Debug, Clone, Component)]
pub struct Stats {
    pub name: String,
    pub race_and_gender: u8,
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
