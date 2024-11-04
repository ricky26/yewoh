use std::cmp::Ordering;

use bevy::prelude::*;
use glam::{IVec2, IVec3};
use serde::{Deserialize, Serialize};

use yewoh::protocol::EquipmentSlot;
use yewoh::Direction;

use crate::math::IVecExt;

#[derive(Clone, Copy, Debug, Default, Deref, DerefMut, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Frozen(pub bool);

#[derive(Clone, Copy, Debug, Default, Deref, DerefMut, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Hidden(pub bool);

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Deref, DerefMut, Component, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hue(pub u16);

#[derive(Debug, Default, Clone, Copy, Deref, DerefMut, Component, Reflect)]
#[reflect(Component, Default)]
pub struct Multi(pub u16);

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Component, Reflect, Serialize, Deserialize)]
#[reflect(Default, Component)]
pub struct MapPosition {
    pub position: IVec3,
    pub map_id: u8,
    #[serde(default)]
    #[reflect(remote = crate::remote_reflect::DirectionRemote)]
    pub direction: Direction,
}

#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, Reflect, Component)]
#[reflect(Component)]
pub struct RootPosition(pub MapPosition);

impl MapPosition {
    pub fn manhattan_distance(&self, other: &MapPosition) -> Option<i32> {
        if self.map_id == other.map_id {
            Some(self.position.truncate().manhattan_distance(&other.position.truncate()))
        } else {
            None
        }
    }

    pub fn in_range(&self, other: &MapPosition, range: i32) -> bool {
        self.manhattan_distance(other).map_or(false, |distance| distance <= range)
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Component, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EquippedPosition {
    #[reflect(remote = crate::remote_reflect::EquipmentSlotRemote)]
    pub slot: EquipmentSlot,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Component, Reflect, Serialize, Deserialize)]
#[reflect(Default, Component, Serialize, Deserialize)]
pub struct ContainedPosition {
    pub position: IVec2,
    pub grid_index: u8,
}

#[derive(Debug, Clone, Eq, PartialEq, Reflect)]
#[reflect(Default)]
pub struct TooltipLine {
    pub text_id: u32,
    pub arguments: String,
    pub priority: u32,
}

impl Default for TooltipLine {
    fn default() -> Self {
        TooltipLine {
            text_id: 1042971,
            arguments: String::new(),
            priority: 0,
        }
    }
}

impl TooltipLine {
    pub fn from_static(text_id: u32, priority: u32) -> TooltipLine {
        Self {
            text_id,
            arguments: Default::default(),
            priority,
        }
    }

    pub fn from_str(text: String, priority: u32) -> TooltipLine {
        Self {
            text_id: 1042971,
            arguments: text,
            priority,
        }
    }
}

impl PartialOrd for TooltipLine {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TooltipLine {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
            .then_with(|| self.text_id.cmp(&other.text_id))
            .then_with(|| self.arguments.cmp(&other.arguments))
    }
}

#[derive(Debug, Clone, Reflect)]
pub struct TooltipRequest {
    pub client: Entity,
    pub entries: Vec<TooltipLine>,
}

#[derive(Debug, Clone, Copy, Default, Component, Reflect)]
#[reflect(Component, Default)]
#[require(TooltipRequests)]
pub struct Tooltip {
    pub version: u32,
}

impl Tooltip {
    pub fn mark_changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component, Default)]
pub struct TooltipRequests {
    pub requests: Vec<TooltipRequest>,
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<Frozen>()
        .register_type::<Hidden>()
        .register_type::<Hue>()
        .register_type::<Multi>()
        .register_type::<MapPosition>()
        .register_type::<RootPosition>()
        .register_type::<ContainedPosition>()
        .register_type::<EquippedPosition>()
        .register_type::<Tooltip>()
        .register_type::<TooltipRequests>()
        .register_type::<TooltipLine>()
        .register_type_data::<Vec<TooltipLine>, ReflectFromReflect>();
}
