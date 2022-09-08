pub mod assets;
pub mod protocol;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd)]
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
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd)]
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
    pub fn from_u8(src: u8) -> Option<Direction> {
        match src {
            0 => Some(Direction::North),
            1 => Some(Direction::Right),
            2 => Some(Direction::East),
            3 => Some(Direction::Down),
            4 => Some(Direction::South),
            5 => Some(Direction::Left),
            6 => Some(Direction::West),
            7 => Some(Direction::Up),
            _ => None,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default)]
pub enum EntityKind {
    #[default]
    Single = 0,
    Multi = 2,
}

impl EntityKind {
    pub fn from_u8(value: u8) -> Option<EntityKind> {
        match value {
            0 => Some(EntityKind::Single),
            2 => Some(EntityKind::Multi),
            _ => None,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default)]
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

impl Notoriety {
    pub fn from_u8(value: u8) -> Option<Notoriety> {
        match value {
            1 => Some(Notoriety::Innocent),
            2 => Some(Notoriety::Friend),
            3 => Some(Notoriety::Neutral),
            4 => Some(Notoriety::Criminal),
            5 => Some(Notoriety::Enemy),
            6 => Some(Notoriety::Murderer),
            7 => Some(Notoriety::Invulnerable),
            _ => None,
        }
    }
}