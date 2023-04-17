use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::*;
use bevy_ecs::system::{Command, EntityCommands};
use bevy_ecs::world::{EntityMut, World};
use bevy_reflect::Reflect;

use yewoh::EntityId;

use crate::world::entity::{Character, Container};

#[derive(Debug, Clone, Copy, Component)]
pub struct NetEntity {
    pub id: EntityId,
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct NetOwner {
    pub client_entity: Entity,
}

#[derive(Debug, Resource)]
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

#[derive(Debug, Default, Resource)]
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

pub fn add_new_entities_to_lookup(
    mut lookup: ResMut<NetEntityLookup>,
    query: Query<(Entity, &NetEntity), Changed<NetEntity>>,
) {
    for (entity, net_entity) in query.iter() {
        lookup.insert(entity, net_entity.id);
    }
}

pub fn remove_old_entities_from_lookup(
    mut lookup: ResMut<NetEntityLookup>,
    mut removals: RemovedComponents<NetEntity>,
) {
    for entity in removals.iter() {
        lookup.remove(entity);
    }
}

pub struct AssignNetId {
    pub entity: Entity,
}

impl Command for AssignNetId {
    fn write(self, world: &mut World) {
        assign_network_id(world, self.entity);
    }
}

pub fn assign_network_id(world: &mut World, entity: Entity) {
    let mut queue = VecDeque::new();
    queue.push_back(entity);

    while let Some(next) = queue.pop_front() {
        if let Some(container) = world.get::<Container>(next) {
            queue.extend(container.items.iter().copied());
        }

        if let Some(character) = world.get::<Character>(next) {
            queue.extend(character.equipment.iter().map(|e| e.entity));
        }

        let allocator = world.resource::<NetEntityAllocator>();
        let entity_info = world.entity(next);
        let id = if entity_info.contains::<Character>() {
            allocator.allocate_character()
        } else {
            allocator.allocate_item()
        };
        world.entity_mut(next).insert(NetEntity { id });
    }
}

pub trait NetCommandsExt {
    fn assign_network_id(&mut self) -> &mut Self;
}

impl<'w, 's, 'a> NetCommandsExt for EntityCommands<'w, 's, 'a> {
    fn assign_network_id(&mut self) -> &mut Self {
        let entity = self.id();
        self.commands().add(AssignNetId { entity });
        self
    }
}

impl<'w> NetCommandsExt for EntityMut<'w> {
    fn assign_network_id(&mut self) -> &mut Self {
        let entity = self.id();
        self.world_scope(|world| assign_network_id(world, entity));
        self
    }
}
