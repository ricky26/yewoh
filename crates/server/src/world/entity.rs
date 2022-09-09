use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};
use bevy_ecs::prelude::*;
use glam::{IVec2, IVec3};

use yewoh::{Direction, EntityId, EntityKind, Notoriety};
use yewoh::protocol::{DeleteEntity, EquipmentSlot, UpsertEntityWorld, UpsertEntityStats, CharacterEquipment, UpsertEntityCharacter, EntityFlags, UpsertEntityEquipped, ContainedItem, UpsertContainerContents};
use crate::world::client::NetClients;

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

#[derive(Debug, Clone, Component)]
pub struct Character {
    pub body_type: u16,
    pub hue: u16,
    pub equipment: Vec<Entity>,
}

#[derive(Debug, Clone, Component)]
pub struct EquippedBy {
    pub entity: Entity,
    pub slot: EquipmentSlot,
}

#[derive(Debug, Clone, Component)]
pub struct Quantity {
    pub quantity: u16,
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
pub struct Container {
    pub gump_id: u16,
    pub items: Vec<Entity>,
}

#[derive(Debug, Clone, Component)]
pub struct ParentContainer {
    pub container: Entity,
    pub position: IVec2,
    pub grid_index: u8,
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
    next_character: AtomicU32,
    next_item: AtomicU32,
}

impl Default for NetEntityAllocator {
    fn default() -> Self {
        Self {
            next_character: AtomicU32::new(1),
            next_item: AtomicU32::new(0x40000001),
        }
    }
}

impl NetEntityAllocator {
    pub fn allocate_character(&self) -> EntityId {
        EntityId::from_u32(self.next_character.fetch_add(1, Ordering::Relaxed))
    }

    pub fn allocate_item(&self) -> EntityId {
        EntityId::from_u32(self.next_item.fetch_add(1, Ordering::Relaxed))
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
    query: Query<(Entity, &NetEntity), Changed<NetEntity>>,
    removals: RemovedComponents<NetEntity>,
) {
    for (entity, net_entity) in query.iter() {
        lookup.insert(entity, net_entity.id);
    }

    for entity in removals.iter() {
        lookup.remove(entity);
    }
}

pub fn send_entity_updates(
    server: Res<NetClients>,
    lookup: Res<NetEntityLookup>,
    world_items_query: Query<
        (&NetEntity, &Graphic, Option<&Quantity>, &MapPosition),
        Or<(Changed<Graphic>, Changed<MapPosition>)>,
    >,
    characters_query: Query<
        (&NetEntity, &Character, &MapPosition, &HasNotoriety),
        Or<(Changed<Character>, Changed<MapPosition>, Changed<HasNotoriety>)>,
    >,
    equipment_query: Query<(&NetEntity, &Graphic, &EquippedBy), Or<(Changed<Graphic>, Changed<EquippedBy>)>>,
    all_equipment_query: Query<(&NetEntity, &Graphic, &EquippedBy)>,
) {
    // TODO: implement an interest system

    for (net, graphic, quantity, position) in world_items_query.iter() {
        let id = net.id;
        server.broadcast_packet(UpsertEntityWorld {
            id,
            kind: EntityKind::Item,
            graphic_id: graphic.id,
            graphic_inc: 0,
            direction: position.direction,
            quantity: quantity.map_or(1, |q| q.quantity),
            position: position.position,
            hue: graphic.hue,
            flags: Default::default(),
        }.into());
    }

    for (net, character, position, notoriety) in characters_query.iter() {
        let entity_id = net.id;
        let hue = character.hue;
        let body_type = character.body_type;
        let mut equipment = Vec::new();

        for child_entity in character.equipment.iter().copied() {
            let (net, graphic, equipped_by) = match all_equipment_query.get(child_entity) {
                Ok(x) => x,
                _ => continue,
            };
            equipment.push(CharacterEquipment {
                id: net.id,
                slot: equipped_by.slot,
                graphic_id: graphic.id,
                hue: graphic.hue,
            });
        }

        server.broadcast_packet(UpsertEntityCharacter {
            id: entity_id,
            body_type,
            position: position.position,
            direction: position.direction,
            hue,
            flags: EntityFlags::empty(),
            notoriety: notoriety.0,
            equipment,
        }.into());
    }

    for (net, graphic, equipped_by) in equipment_query.iter() {
        let parent_id = match lookup.ecs_to_net(equipped_by.entity) {
            Some(x) => x,
            None => continue,
        };

        server.broadcast_packet(UpsertEntityEquipped {
            id: net.id,
            parent_id,
            slot: equipped_by.slot,
            graphic_id: graphic.id,
            hue: graphic.hue,
        }.into());
    }
}

pub fn update_containers_with_updated_items(
    item_query: Query<&ParentContainer, Or<(Changed<Graphic>, Changed<Quantity>, Changed<ParentContainer>)>>,
    containers: Query<&mut Container>,
) {
    for parent_container in item_query.iter() {
        // Just accessing this is enough to trigger change detection.
        let _ = containers.get(parent_container.container);
    }
}

pub fn send_updated_container_contents(
    clients: Res<NetClients>,
    container_query: Query<(&NetEntity, &Container), Changed<Container>>,
    content_query: Query<(&NetEntity, &ParentContainer, &Graphic, Option<&Quantity>)>,
) {
    for (net, container) in container_query.iter() {
        clients.broadcast_packet(make_container_contents_packet(net.id, container, &content_query).into());
    }
}

pub fn make_container_contents_packet(
    id: EntityId, container: &Container,
    content_query: &Query<(&NetEntity, &ParentContainer, &Graphic, Option<&Quantity>)>,
) -> UpsertContainerContents {
    let mut items = Vec::with_capacity(container.items.len());

    for item in container.items.iter() {
        let item = *item;
        let (net_id, parent, graphic, quantity) = match content_query.get(item) {
            Ok(x) => x,
            _ => continue,
        };

        items.push(ContainedItem {
            id: net_id.id,
            graphic_id: graphic.id,
            quantity: quantity.map_or(1, |q| q.quantity),
            position: parent.position,
            grid_index: parent.grid_index,
            container_id: id,
            hue: graphic.hue,
        });
    }

    UpsertContainerContents {
        items,
    }
}

pub fn send_remove_entity(
    server: Res<NetClients>,
    lookup: Res<NetEntityLookup>,
    removals: RemovedComponents<NetEntity>,
) {
    for entity in removals.iter() {
        let id = match lookup.ecs_to_net(entity) {
            Some(x) => x,
            None => continue,
        };

        server.broadcast_packet(DeleteEntity { id }.into());
    }
}

pub fn send_updated_stats(
    server: Res<NetClients>,
    query: Query<(&NetEntity, &Stats), Changed<Stats>>,
) {
    for (net, stats) in query.iter() {
        server.broadcast_packet(stats.upsert(net.id, true).into());
    }
}