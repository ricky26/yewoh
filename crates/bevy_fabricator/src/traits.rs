use bevy::prelude::*;

#[reflect_trait]
pub trait Evaluate {
    fn evaluate(&self, world: &mut World) -> anyhow::Result<Box<dyn PartialReflect>>;
}

#[reflect_trait]
pub trait Apply {
    fn apply(&self, world: &mut World, entity: Entity) -> anyhow::Result<()>;
}
