use bevy_ecs::entity::Entity;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::world::{Command, EntityWorldMut, World};

use crate::world::entity::{Character, Container};

pub struct DespawnRecursive {
    pub entity: Entity,
}

impl Command for DespawnRecursive {
    fn apply(self, world: &mut World) {
        despawn_recursive(world, self.entity);
    }
}

pub fn despawn_recursive(world: &mut World, entity: Entity) {
    if let Some(mut container) = world.get_mut::<Container>(entity) {
        for child_entity in std::mem::take(&mut container.items) {
            despawn_recursive(world, child_entity);
        }
    }

    if let Some(mut character) = world.get_mut::<Character>(entity) {
        for equipped in std::mem::take(&mut character.equipment) {
            despawn_recursive(world, equipped.entity);
        }
    }

    world.despawn(entity);
}

pub trait DespawnRecursiveExt {
    fn despawn_recursive(self);
}

impl<'w> DespawnRecursiveExt for EntityCommands<'w> {
    fn despawn_recursive(mut self) {
        let entity = self.id();
        self.commands().add(DespawnRecursive { entity });
    }
}

impl<'w> DespawnRecursiveExt for EntityWorldMut<'w> {
    fn despawn_recursive(self) {
        let entity = self.id();
        despawn_recursive(self.into_world_mut(), entity);
    }
}
