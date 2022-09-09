use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};
use bevy_ecs::prelude::*;
use glam::IVec3;

use yewoh::{Direction, EntityId, Notoriety};
use yewoh::protocol::{EquipmentSlot, UpsertEntityStats};

#[derive(Debug, Clone, Copy, Component)]
pub struct NetEntity {
    pub id: EntityId,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct NetOwner {
    pub connection: Entity,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct HasNotoriety(pub Notoriety);

impl Deref for HasNotoriety {
    type Target = Notoriety;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for HasNotoriety {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

#[derive(Debug, Clone)]
pub struct Equipment {
    pub slot: EquipmentSlot,
    pub entity: Entity,
}

#[derive(Debug, Clone, Component)]
pub struct Character {
    pub body_type: u16,
    pub hue: u16,
    pub equipment: Vec<Equipment>,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct Graphic {
    pub id: u16,
    pub hue: u16,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct Multi {
    pub id: u16,
}

#[derive(Debug, Clone, Copy, Component, Default)]
pub struct MapPosition {
    pub position: IVec3,
    pub map_id: u8,
    pub direction: Direction,
}

#[derive(Debug, Clone, Default, Component)]
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

impl Stats {
    pub fn upsert(&self, id: EntityId, owned: bool) -> UpsertEntityStats {
        let max_info_level = if owned { 8 } else { 0 };
        UpsertEntityStats {
            id,
            max_info_level,
            name: self.name.clone(),
            allow_name_change: owned,
            race_and_gender: self.race_and_gender,
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

#[derive(Debug)]
pub struct NetEntityAllocator {
    next_id: AtomicU32,
}

impl Default for NetEntityAllocator {
    fn default() -> Self {
        Self {
            next_id: AtomicU32::new(1),
        }
    }
}

impl NetEntityAllocator {
    pub fn allocate(&self) -> EntityId {
        EntityId::from_u32(self.next_id.fetch_add(1, Ordering::Relaxed))
    }
}


#[derive(Debug, Default)]
pub struct NetEntityLookup {
    net_to_ecs: HashMap<EntityId, Entity>,
    ecs_to_net: HashMap<Entity, EntityId>,
}

impl NetEntityLookup {
    pub fn net_to_ecs(&self, id: EntityId) -> Option<Entity> {
        self.net_to_ecs.get(&id).copied()
    }

    pub fn ecs_to_net(&self, entity: Entity) -> Option<EntityId> {
        self.ecs_to_net.get(&entity).copied()
    }

    pub fn insert(&mut self, entity: Entity, id: EntityId) {
        if let Some(old_id) = self.ecs_to_net.get(&entity) {
            if id == *old_id {
                return;
            }

            self.net_to_ecs.remove(old_id);
        }

        self.net_to_ecs.insert(id, entity);
        self.ecs_to_net.insert(entity, id);
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some(id) = self.ecs_to_net.remove(&entity) {
            self.net_to_ecs.remove(&id);
        }
    }
}

pub fn update_entity_lookup(
    mut lookup: ResMut<NetEntityLookup>,
    query: Query<(Entity, &NetEntity), Or<(Added<NetEntity>, Changed<NetEntity>)>>,
    removals: RemovedComponents<NetEntity>,
) {
    for (entity, net_entity) in query.iter() {
        lookup.insert(entity, net_entity.id);
    }

    for entity in removals.iter() {
        lookup.remove(entity);
    }
}
