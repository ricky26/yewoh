use bevy::prelude::*;

#[reflect_trait]
pub trait Evaluate {
    fn evaluate(&self, world: &mut World) -> Box<dyn PartialReflect>;
}

#[reflect_trait]
pub trait Apply {
    fn apply(&self, world: &mut World, entity: Entity);
}
