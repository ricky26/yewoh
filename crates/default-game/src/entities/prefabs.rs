use bevy::prelude::*;
use bevy_fabricator::Fabricated;
use yewoh::protocol::EquipmentSlot;
use bevy_fabricator::traits::{Apply, ReflectApply};
use yewoh::Direction;
use yewoh_server::world::entity::{ContainerPosition, MapPosition};
use crate::data::prefabs::PrefabLibraryEntityExt;
use crate::entities::position::PositionExt;

#[derive(Clone, Debug, Reflect)]
#[reflect(Apply)]
pub struct Prefab(pub String);

impl Apply for Prefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        world.entity_mut(entity).fabricate_prefab(&self.0);
        Ok(())
    }
}

#[derive(Clone, Debug, Reflect)]
#[reflect(Apply)]
pub struct EquippedBy {
    pub parent: Entity,
    #[reflect(remote = yewoh_server::remote_reflect::EquipmentSlotRemote)]
    pub slot: EquipmentSlot,
}

impl Apply for EquippedBy {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        world.entity_mut(entity).move_to_equipped_position(self.parent, self.slot);
        Ok(())
    }
}

#[derive(Clone, Debug, Reflect)]
#[reflect(Apply)]
pub struct ContainedBy {
    pub parent: Entity,
    #[reflect(default)]
    pub position: ContainerPosition,
}

impl Apply for ContainedBy {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        world.entity_mut(entity).move_to_container_position(self.parent, self.position);
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Reflect)]
#[reflect(Default, Apply)]
pub struct AtMapPosition {
    pub position: IVec3,
    pub map_id: u8,
    #[reflect(remote = yewoh_server::remote_reflect::DirectionRemote)]
    pub direction: Direction,
}

impl Apply for AtMapPosition {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        let position = MapPosition {
            position: self.position,
            map_id: self.map_id,
            direction: self.direction,
        };
        world.entity_mut(entity).move_to_map_position(position);
        Ok(())
    }
}
