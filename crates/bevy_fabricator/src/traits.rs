use bevy::prelude::*;
use bevy::reflect::FromType;
use crate::Fabricated;

pub struct Context<'w> {
    pub world: &'w mut World,
    pub fabricated: Fabricated,
}

pub trait Convert {
    fn convert(from: Box<dyn PartialReflect>) -> anyhow::Result<Box<dyn PartialReflect>>;
}

pub type ConvertFn = fn(Box<dyn PartialReflect>) -> anyhow::Result<Box<dyn PartialReflect>>;

#[derive(Clone)]
pub struct ReflectConvert {
    convert: ConvertFn,
}

impl<T: Convert> FromType<T> for ReflectConvert {
    fn from_type() -> Self {
        ReflectConvert {
            convert: T::convert,
        }
    }
}

impl ReflectConvert {
    pub fn new(convert: ConvertFn) -> ReflectConvert {
        ReflectConvert { convert }
    }

    pub fn convert(
        &self, value: Box<dyn PartialReflect>,
    ) -> anyhow::Result<Box<dyn PartialReflect>> {
        (self.convert)(value)
    }
}

#[reflect_trait]
pub trait Evaluate {
    fn evaluate(&self, ctx: &mut Context<'_>) -> anyhow::Result<Box<dyn PartialReflect>>;
}

#[reflect_trait]
pub trait Apply {
    fn apply(&self, ctx: &mut Context<'_>, entity: Entity) -> anyhow::Result<()>;
}
