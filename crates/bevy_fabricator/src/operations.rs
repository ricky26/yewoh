use bevy::prelude::*;
use crate::traits::{Evaluate, ReflectEvaluate};

#[derive(Clone, Default, Reflect)]
#[reflect(Default, Evaluate)]
pub struct Spawn;

impl Evaluate for Spawn {
    fn evaluate(&self, world: &mut World) -> Box<dyn PartialReflect> {
        Box::new(world.spawn_empty().id())
    }
}
