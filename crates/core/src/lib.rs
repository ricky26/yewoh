
pub mod assets;
pub mod protocol;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default)]
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
