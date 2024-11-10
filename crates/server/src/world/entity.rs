use bevy::prelude::*;
use glam::{IVec2, IVec3};
use rand::distributions::Distribution;
use rand::Rng;
use serde::{Deserialize, Serialize};
use yewoh::protocol;
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Reflect, Serialize, Deserialize)]
#[reflect(Default, Serialize, Deserialize)]
pub enum Direction {
    #[default]
    North,
    Right,
    East,
    Down,
    South,
    Left,
    West,
    Up,
}

impl From<yewoh::Direction> for Direction {
    fn from(value: yewoh::Direction) -> Self {
        match value {
            yewoh::Direction::North => Direction::North,
            yewoh::Direction::Right => Direction::Right,
            yewoh::Direction::East => Direction::East,
            yewoh::Direction::Down => Direction::Down,
            yewoh::Direction::South => Direction::South,
            yewoh::Direction::Left => Direction::Left,
            yewoh::Direction::West => Direction::West,
            yewoh::Direction::Up => Direction::Up,
        }
    }
}

impl From<Direction> for yewoh::Direction {
    fn from(value: Direction) -> Self {
        match value {
            Direction::North => yewoh::Direction::North,
            Direction::Right => yewoh::Direction::Right,
            Direction::East => yewoh::Direction::East,
            Direction::Down => yewoh::Direction::Down,
            Direction::South => yewoh::Direction::South,
            Direction::Left => yewoh::Direction::Left,
            Direction::West => yewoh::Direction::West,
            Direction::Up => yewoh::Direction::Up,
        }
    }
}

impl Direction {
    pub fn as_vec2(self) -> IVec2 {
        match self {
            Direction::North => IVec2::new(0, -1),
            Direction::Right => IVec2::new(1, -1),
            Direction::East => IVec2::new(1, 0),
            Direction::Down => IVec2::new(1, 1),
            Direction::South => IVec2::new(0, 1),
            Direction::Left => IVec2::new(-1, 1),
            Direction::West => IVec2::new(-1, 0),
            Direction::Up => IVec2::new(-1, -1),
        }
    }
}

impl Distribution<Direction> for rand::distributions::Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Direction {
        match rng.gen_range(0..8) {
            0 => Direction::North,
            1 => Direction::Right,
            2 => Direction::East,
            3 => Direction::Down,
            4 => Direction::South,
            5 => Direction::Left,
            6 => Direction::West,
            7 => Direction::Up,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialOrd, Ord, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[reflect(Default, Serialize, Deserialize)]
pub enum EquipmentSlot {
    #[default]
    MainHand,
    OffHand,
    Shoes,
    Bottom,
    Top,
    Head,
    Hands,
    Ring,
    Talisman,
    Neck,
    Hair,
    Waist,
    InnerTorso,
    Bracelet,
    FacialHair,
    MiddleTorso,
    Earrings,
    Arms,
    Cloak,
    Backpack,
    OuterTorso,
    OuterLegs,
    InnerLegs,
    Mount,
    ShopBuy,
    ShopBuyback,
    ShopSell,
    Bank,
}

impl EquipmentSlot {
    pub fn from_protocol(slot: protocol::EquipmentSlot) -> Option<EquipmentSlot> {
        match slot {
            protocol::EquipmentSlot::Invalid => None,
            protocol::EquipmentSlot::MainHand => Some(EquipmentSlot::MainHand),
            protocol::EquipmentSlot::OffHand => Some(EquipmentSlot::OffHand),
            protocol::EquipmentSlot::Shoes => Some(EquipmentSlot::Shoes),
            protocol::EquipmentSlot::Bottom => Some(EquipmentSlot::Bottom),
            protocol::EquipmentSlot::Top => Some(EquipmentSlot::Top),
            protocol::EquipmentSlot::Head => Some(EquipmentSlot::Head),
            protocol::EquipmentSlot::Hands => Some(EquipmentSlot::Hands),
            protocol::EquipmentSlot::Ring => Some(EquipmentSlot::Ring),
            protocol::EquipmentSlot::Talisman => Some(EquipmentSlot::Talisman),
            protocol::EquipmentSlot::Neck => Some(EquipmentSlot::Neck),
            protocol::EquipmentSlot::Hair => Some(EquipmentSlot::Hair),
            protocol::EquipmentSlot::Waist => Some(EquipmentSlot::Waist),
            protocol::EquipmentSlot::InnerTorso => Some(EquipmentSlot::InnerTorso),
            protocol::EquipmentSlot::Bracelet => Some(EquipmentSlot::Bracelet),
            protocol::EquipmentSlot::FacialHair => Some(EquipmentSlot::FacialHair),
            protocol::EquipmentSlot::MiddleTorso => Some(EquipmentSlot::MiddleTorso),
            protocol::EquipmentSlot::Earrings => Some(EquipmentSlot::Earrings),
            protocol::EquipmentSlot::Arms => Some(EquipmentSlot::Arms),
            protocol::EquipmentSlot::Cloak => Some(EquipmentSlot::Cloak),
            protocol::EquipmentSlot::Backpack => Some(EquipmentSlot::Backpack),
            protocol::EquipmentSlot::OuterTorso => Some(EquipmentSlot::OuterTorso),
            protocol::EquipmentSlot::OuterLegs => Some(EquipmentSlot::OuterLegs),
            protocol::EquipmentSlot::InnerLegs => Some(EquipmentSlot::InnerLegs),
            protocol::EquipmentSlot::Mount => Some(EquipmentSlot::Mount),
            protocol::EquipmentSlot::ShopBuy => Some(EquipmentSlot::ShopBuy),
            protocol::EquipmentSlot::ShopBuyback => Some(EquipmentSlot::ShopBuyback),
            protocol::EquipmentSlot::ShopSell => Some(EquipmentSlot::ShopSell),
            protocol::EquipmentSlot::Bank => Some(EquipmentSlot::Bank),
        }
    }
}

impl From<EquipmentSlot> for protocol::EquipmentSlot {
    fn from(value: EquipmentSlot) -> Self {
        match value {
            EquipmentSlot::MainHand => protocol::EquipmentSlot::MainHand,
            EquipmentSlot::OffHand => protocol::EquipmentSlot::OffHand,
            EquipmentSlot::Shoes => protocol::EquipmentSlot::Shoes,
            EquipmentSlot::Bottom => protocol::EquipmentSlot::Bottom,
            EquipmentSlot::Top => protocol::EquipmentSlot::Top,
            EquipmentSlot::Head => protocol::EquipmentSlot::Head,
            EquipmentSlot::Hands => protocol::EquipmentSlot::Hands,
            EquipmentSlot::Ring => protocol::EquipmentSlot::Ring,
            EquipmentSlot::Talisman => protocol::EquipmentSlot::Talisman,
            EquipmentSlot::Neck => protocol::EquipmentSlot::Neck,
            EquipmentSlot::Hair => protocol::EquipmentSlot::Hair,
            EquipmentSlot::Waist => protocol::EquipmentSlot::Waist,
            EquipmentSlot::InnerTorso => protocol::EquipmentSlot::InnerTorso,
            EquipmentSlot::Bracelet => protocol::EquipmentSlot::Bracelet,
            EquipmentSlot::FacialHair => protocol::EquipmentSlot::FacialHair,
            EquipmentSlot::MiddleTorso => protocol::EquipmentSlot::MiddleTorso,
            EquipmentSlot::Earrings => protocol::EquipmentSlot::Earrings,
            EquipmentSlot::Arms => protocol::EquipmentSlot::Arms,
            EquipmentSlot::Cloak => protocol::EquipmentSlot::Cloak,
            EquipmentSlot::Backpack => protocol::EquipmentSlot::Backpack,
            EquipmentSlot::OuterTorso => protocol::EquipmentSlot::OuterTorso,
            EquipmentSlot::OuterLegs => protocol::EquipmentSlot::OuterLegs,
            EquipmentSlot::InnerLegs => protocol::EquipmentSlot::InnerLegs,
            EquipmentSlot::Mount => protocol::EquipmentSlot::Mount,
            EquipmentSlot::ShopBuy => protocol::EquipmentSlot::ShopBuy,
            EquipmentSlot::ShopBuyback => protocol::EquipmentSlot::ShopBuyback,
            EquipmentSlot::ShopSell => protocol::EquipmentSlot::ShopSell,
            EquipmentSlot::Bank => protocol::EquipmentSlot::Bank,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Component, Reflect, Serialize, Deserialize)]
#[reflect(Default, Component, Serialize, Deserialize)]
pub struct MapPosition {
    pub position: IVec3,
    pub map_id: u8,
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
#[reflect(Default, Component, Serialize, Deserialize)]
pub struct EquippedPosition {
    pub slot: EquipmentSlot,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Component, Reflect, Serialize, Deserialize)]
#[reflect(Default, Component, Serialize, Deserialize)]
pub struct ContainedPosition {
    pub position: IVec2,
    pub grid_index: u8,
}

#[derive(Debug, Clone, Reflect, Event)]
pub struct OnClientTooltipRequest {
    pub client_entity: Entity,
    pub targets: Vec<Entity>,
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
    pub requests: Vec<OnClientTooltipRequest>,
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
        .add_event::<OnClientTooltipRequest>();
}
