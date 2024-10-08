use std::collections::VecDeque;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{EntityCommands};
use bevy::ecs::world::{Command, EntityWorldMut, World};
use yewoh_server::world::entity::{Character, Container};
use crate::entities::Persistent;

pub struct ChangePersistence {
    pub entity: Entity,
    pub persistent: bool,
}

impl Command for ChangePersistence {
    fn apply(self, world: &mut World) {
        set_persistent(world, self.entity, self.persistent);
    }
}

pub fn set_persistent(world: &mut World, entity: Entity, persistent: bool) {
    let mut queue = VecDeque::new();
    queue.push_back(entity);

    while let Some(next) = queue.pop_front() {
        if let Some(container) = world.get::<Container>(next) {
            queue.extend(container.items.iter().copied());
        }

        if let Some(character) = world.get::<Character>(next) {
            queue.extend(character.equipment.iter().map(|e| e.entity));
        }

        if persistent {
            world.entity_mut(entity).insert(Persistent);
        } else {
            world.entity_mut(entity).remove::<Persistent>();
        }
    }
}

pub trait PersistenceCommandsExt {
    fn change_persistence(&mut self, persistent: bool) -> &mut Self;

    fn make_persistent(&mut self) -> &mut Self {
        self.change_persistence(true)
    }

    fn make_transient(&mut self) -> &mut Self {
        self.change_persistence(false)
    }
}

impl<'w> PersistenceCommandsExt for EntityCommands<'w> {
    fn change_persistence(&mut self, persistent: bool) -> &mut Self {
        let entity = self.id();
        self.commands().add(ChangePersistence { entity, persistent });
        self
    }
}

impl<'w> PersistenceCommandsExt for EntityWorldMut<'w> {
    fn change_persistence(&mut self, persistent: bool) -> &mut Self {
        let entity = self.id();
        self.world_scope(|world| set_persistent(world, entity, persistent));
        self
    }
}
