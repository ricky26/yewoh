use glam::IVec2;
use strum_macros::FromRepr;

pub mod assets;
pub mod protocol;

#[derive(Debug, Clone, Copy, Default, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct EntityId(u32);

impl EntityId {
    pub const ZERO: EntityId = EntityId::from_u32(0);

    pub fn is_valid(&self) -> bool { *self != Self::ZERO }

    pub fn as_u32(&self) -> u32 { self.0 }

    pub const fn from_u32(value: u32) -> EntityId {
        Self(value)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, FromRepr)]
pub enum Direction {
    #[default]
    North = 0,
    Right = 1,
    East = 2,
    Down = 3,
    South = 4,
    Left = 5,
    West = 6,
    Up = 7,
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, FromRepr)]
pub enum EntityKind {
    #[default]
    Item = 0,
    Character = 1,
    Multi = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, FromRepr)]
pub enum Notoriety {
    #[default]
    Innocent = 1,
    Friend = 2,
    Neutral = 3,
    Criminal = 4,
    Enemy = 5,
    Murderer = 6,
    Invulnerable = 7,
}
