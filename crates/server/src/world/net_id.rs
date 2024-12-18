use bevy::ecs::component::{ComponentHooks, StorageType};
use bevy::prelude::*;
use bevy::reflect::std_traits::ReflectDefault;
use bevy::reflect::Reflect;
use std::collections::HashMap;

use yewoh::{EntityId, MIN_ITEM_ID};
use crate::world::characters::CharacterBodyType;
use crate::world::items::ItemGraphic;
use crate::world::map::Static;
use crate::world::ServerSet;

#[derive(Debug, Clone, Copy, Default, Reflect)]
#[reflect(opaque, Default, Debug, Component)]
pub struct NetId {
    pub id: EntityId,
}

impl From<EntityId> for NetId {
    fn from(id: EntityId) -> Self {
        NetId {
            id,
        }
    }
}

impl Component for NetId {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, entity, _| {
            let id = world.get::<NetId>(entity).unwrap().id;
            let mut lookup = world.resource_mut::<NetEntityLookup>();
            if let Some(prev) = lookup.net_to_ecs.insert(id, entity) {
                warn!("duplicate net ID assigned: {id:?} ({prev:?} & {entity:?})");
            }
        });
        hooks.on_remove(|mut world, entity, _| {
            let id = world.get::<NetId>(entity).unwrap().id;
            {
                let mut lookup = world.resource_mut::<NetEntityLookup>();
                if lookup.net_to_ecs.remove(&id).is_none() {
                    warn!("duplicate net ID removal: {id:?} (entity={entity:?})");
                }
            }
            {
                let mut removed = world.resource_mut::<Events<OnDestroyNetEntity>>();
                removed.send(OnDestroyNetEntity { entity, id });
            }
        });
    }
}

#[derive(Clone, Debug, Event)]
pub struct OnDestroyNetEntity {
    pub entity: Entity,
    pub id: EntityId,
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Default, Component)]
pub struct CharacterNetId;

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Default, Component)]
pub struct ItemNetId;

#[derive(Debug, Resource)]
pub struct NetIdAllocator {
    next_character: u32,
    next_item: u32,
}

impl Default for NetIdAllocator {
    fn default() -> Self {
        Self {
            next_character: 0,
            next_item: MIN_ITEM_ID,
        }
    }
}

impl NetIdAllocator {
    pub fn allocate_character(&mut self) -> EntityId {
        self.next_character += 1;
        if self.next_character >= MIN_ITEM_ID {
            panic!("character ID overflow");
        }
        EntityId::from_u32(self.next_character)
    }

    pub fn allocate_item(&mut self) -> EntityId {
        self.next_item += 1;
        EntityId::from_u32(self.next_item)
    }
}

#[derive(Debug, Default, Resource)]
pub struct NetEntityLookup {
    net_to_ecs: HashMap<EntityId, Entity>,
}

impl NetEntityLookup {
    pub fn net_to_ecs(&self, id: EntityId) -> Option<Entity> {
        self.net_to_ecs.get(&id).copied()
    }
}

pub fn assign_net_ids(
    mut commands: Commands,
    mut id_allocator: ResMut<NetIdAllocator>,
    new_characters: Query<Entity, (Without<Static>, With<CharacterBodyType>, Without<ItemGraphic>, Without<CharacterNetId>)>,
    new_items: Query<Entity, (Without<Static>, With<ItemGraphic>, Without<CharacterBodyType>, Without<ItemNetId>)>,
) {
    for entity in &new_characters {
        commands.entity(entity)
            .remove::<ItemNetId>()
            .insert((
                NetId::from(id_allocator.allocate_character()),
                CharacterNetId,
            ));
    }

    for entity in &new_items {
        commands.entity(entity)
            .remove::<CharacterNetId>()
            .insert((
                NetId::from(id_allocator.allocate_item()),
                ItemNetId,
            ));
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<NetId>()
        .add_event::<OnDestroyNetEntity>()
        .init_resource::<NetIdAllocator>()
        .init_resource::<NetEntityLookup>()
        .add_systems(Last, (
            assign_net_ids,
        ).in_set(ServerSet::AssignNetIds));
}
