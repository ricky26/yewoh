use std::ops::Sub;
use glam::{IVec2, IVec3};

pub trait IVecExt : Copy + Sized + Sub<Self, Output=Self> {
    fn manhattan_magnitude(&self) -> i32;

    fn manhattan_distance(&self, other: &Self) -> i32 {
        (*self - *other).manhattan_magnitude()
    }

    fn in_range(&self, other: &Self, range: i32) -> bool {
        self.manhattan_distance(other) as i32 <= range
    }
}

impl IVecExt for IVec2 {
    fn manhattan_magnitude(&self) -> i32 {
        self.x.abs() + self.y.abs()
    }
}

impl IVecExt for IVec3 {
    fn manhattan_magnitude(&self) -> i32 {
        self.x.abs() + self.y.abs() + self.z.abs()
    }
}
