use bevy::prelude::*;
use bevy_fabricator::traits::{Apply, Context, ReflectApply};
use yewoh_server::world::entity::{ContainedPosition, Direction, EquipmentSlot, MapPosition};
use crate::data::prefabs::PrefabLibraryEntityExt;
use crate::entities::position::PositionExt;

#[derive(Clone, Debug, Reflect)]
#[reflect(Apply)]
pub struct Prefab(pub String);

impl Apply for Prefab {
    fn apply(&self, ctx: &mut Context, entity: Entity) -> anyhow::Result<()> {
        ctx.world.entity_mut(entity).fabricate_prefab(&self.0);
        Ok(())
    }
}

#[derive(Clone, Debug, Reflect)]
#[reflect(Apply)]
pub struct EquippedBy {
    pub parent: Entity,
    pub slot: EquipmentSlot,
}

impl Apply for EquippedBy {
    fn apply(&self, ctx: &mut Context, entity: Entity) -> anyhow::Result<()> {
        ctx.world.entity_mut(entity).move_to_equipped_position(self.parent, self.slot);
        Ok(())
    }
}

#[derive(Clone, Debug, Reflect)]
#[reflect(Apply)]
pub struct ContainedBy {
    pub parent: Entity,
    #[reflect(default)]
    pub position: ContainedPosition,
}

impl Apply for ContainedBy {
    fn apply(&self, ctx: &mut Context, entity: Entity) -> anyhow::Result<()> {
        ctx.world.entity_mut(entity).move_to_container_position(self.parent, self.position);
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Reflect)]
#[reflect(Default, Apply)]
pub struct AtMapPosition {
    pub position: IVec3,
    pub map_id: u8,
    pub direction: Direction,
}

impl Apply for AtMapPosition {
    fn apply(&self, ctx: &mut Context, entity: Entity) -> anyhow::Result<()> {
        let position = MapPosition {
            position: self.position,
            map_id: self.map_id,
            direction: self.direction,
        };
        ctx.world.entity_mut(entity).move_to_map_position(position);
        Ok(())
    }
}
