use serde::{Deserialize, Serialize};
use strum_macros::FromRepr;

pub mod assets;
pub mod protocol;
pub mod types;

pub const MIN_ITEM_ID: u32 = 0x40000000;

#[derive(Debug, Clone, Copy, Default, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct EntityId(u32);

impl EntityId {
    pub const ZERO: EntityId = EntityId::from_u32(0);

    pub fn is_valid(&self) -> bool { *self != Self::ZERO }

    pub fn is_item(&self) -> bool {
        self.0 >= MIN_ITEM_ID
    }

    pub fn as_u32(&self) -> u32 { self.0 }

    pub const fn from_u32(value: u32) -> EntityId {
        Self(value)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, FromRepr, Serialize, Deserialize)]
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, FromRepr)]
pub enum EntityKind {
    #[default]
    Item = 0,
    Character = 1,
    Multi = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, FromRepr, Serialize, Deserialize)]
pub enum Notoriety {
    Invalid = 0,
    Innocent = 1,
    Ally = 2,
    #[default]
    Neutral = 3,
    Criminal = 4,
    Enemy = 5,
    Murderer = 6,
    Invulnerable = 7,
}
