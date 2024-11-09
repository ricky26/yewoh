use bevy::prelude::*;
use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::entity::{ContainedPosition, EquippedPosition, MapPosition};
use yewoh_server::world::items::ItemPosition;

pub struct MoveToMapPosition {
    pub map_position: MapPosition,
}

impl EntityCommand for MoveToMapPosition {
    fn apply(self, entity: Entity, world: &mut World) {
        world.entity_mut(entity)
            .remove_parent()
            .remove::<(ContainedPosition, EquippedPosition)>()
            .insert(self.map_position);
    }
}

pub struct MoveToEquippedPosition {
    pub parent: Entity,
    pub slot: EquipmentSlot,
}

impl EntityCommand for MoveToEquippedPosition {
    fn apply(self, entity: Entity, world: &mut World) {
        world.entity_mut(entity)
            .set_parent(self.parent)
            .remove::<(ContainedPosition, MapPosition)>()
            .insert(EquippedPosition { slot: self.slot });
    }
}

pub struct MoveToContainerPosition {
    pub parent: Entity,
    pub position: ContainedPosition,
}

impl EntityCommand for MoveToContainerPosition {
    fn apply(self, entity: Entity, world: &mut World) {
        world.entity_mut(entity)
            .set_parent(self.parent)
            .remove::<(EquippedPosition, MapPosition)>()
            .insert(self.position);
    }
}

pub struct RemovePosition;

impl EntityCommand for RemovePosition {
    fn apply(self, entity: Entity, world: &mut World) {
        world.entity_mut(entity)
            .remove_parent()
            .remove::<(ContainedPosition, EquippedPosition, MapPosition)>();
    }
}

pub trait PositionExt {
    fn move_to_map_position(&mut self, map_position: MapPosition) -> &mut Self;

    fn move_to_equipped_position(&mut self, parent: Entity, slot: EquipmentSlot) -> &mut Self;

    fn move_to_container_position(
        &mut self, parent: Entity, position: ContainedPosition,
    ) -> &mut Self;

    fn remove_position(&mut self) -> &mut Self;

    fn move_to_item_position(&mut self, position: ItemPosition) -> &mut Self {
        match position {
            ItemPosition::Map(map_position) =>
                self.move_to_map_position(map_position),
            ItemPosition::Equipped(parent, equipped) =>
                self.move_to_equipped_position(parent, equipped.slot),
            ItemPosition::Contained(parent, contained) =>
                self.move_to_container_position(parent, contained),
        }
    }
}

impl PositionExt for EntityCommands<'_> {
    fn move_to_map_position(&mut self, map_position: MapPosition) -> &mut Self {
        self.queue(MoveToMapPosition { map_position })
    }

    fn move_to_equipped_position(&mut self, parent: Entity, slot: EquipmentSlot) -> &mut Self {
        self.queue(MoveToEquippedPosition { parent, slot })
    }

    fn move_to_container_position(
        &mut self, parent: Entity, position: ContainedPosition,
    ) -> &mut Self {
        self.queue(MoveToContainerPosition { parent, position })
    }

    fn remove_position(&mut self) -> &mut Self {
        self.queue(RemovePosition)
    }
}

fn entity_world_apply<'a, 'w>(
    this: &'a mut EntityWorldMut<'w>, command: impl EntityCommand,
) -> &'a mut EntityWorldMut<'w> {
    let entity = this.id();
    this.world_scope(move |world| command.apply(entity, world));
    this
}

impl PositionExt for EntityWorldMut<'_> {
    fn move_to_map_position(&mut self, map_position: MapPosition) -> &mut Self {
        entity_world_apply(self, MoveToMapPosition { map_position })
    }

    fn move_to_equipped_position(&mut self, parent: Entity, slot: EquipmentSlot) -> &mut Self {
        entity_world_apply(self, MoveToEquippedPosition { parent, slot })
    }

    fn move_to_container_position(
        &mut self, parent: Entity, position: ContainedPosition,
    ) -> &mut Self {
        entity_world_apply(self, MoveToContainerPosition { parent, position })
    }

    fn remove_position(&mut self) -> &mut Self {
        entity_world_apply(self, RemovePosition)
    }
}
