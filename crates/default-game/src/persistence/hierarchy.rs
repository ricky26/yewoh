use std::collections::VecDeque;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Command, EntityCommands};
use bevy_ecs::world::{EntityMut, World};
use yewoh_server::world::entity::{Character, Container};
use crate::entities::Persistent;

pub struct ChangePersistence {
    pub entity: Entity,
    pub persistent: bool,
}

impl Command for ChangePersistence {
    fn write(self, world: &mut World) {
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
            queue.extend(character.equipment.iter().map(|e| e.equipment));
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

impl<'w, 's, 'a> PersistenceCommandsExt for EntityCommands<'w, 's, 'a> {
    fn change_persistence(&mut self, persistent: bool) -> &mut Self {
        let entity = self.id();
        self.commands().add(ChangePersistence { entity, persistent });
        self
    }
}

impl<'w> PersistenceCommandsExt for EntityMut<'w> {
    fn change_persistence(&mut self, persistent: bool) -> &mut Self {
        let entity = self.id();
        self.world_scope(|world| set_persistent(world, entity, persistent));
        self
    }
}
