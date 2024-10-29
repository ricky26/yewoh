use bevy::prelude::*;
use crate::Fabricated;

#[reflect_trait]
pub trait Evaluate {
    fn evaluate(
        &self, world: &mut World, fabricated: &mut Fabricated,
    ) -> anyhow::Result<Box<dyn PartialReflect>>;
}

#[reflect_trait]
pub trait Apply {
    fn apply(
        &self, world: &mut World, entity: Entity, fabricated: &mut Fabricated,
    ) -> anyhow::Result<()>;
}
