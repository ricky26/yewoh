use bevy::prelude::*;
use bevy::reflect::{DynamicEnum, DynamicTuple, DynamicVariant};
use crate::any::Any;
use crate::Fabricated;
use crate::traits::{Evaluate, ReflectEvaluate};

#[derive(Default, Reflect)]
#[reflect(Default, Evaluate)]
pub struct None;

impl Evaluate for None {
    fn evaluate(
        &self, _world: &mut World, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<Box<dyn PartialReflect>> {
        Ok(Box::new(DynamicEnum::new("None", DynamicVariant::Unit)))
    }
}

#[derive(Reflect)]
#[reflect(Evaluate)]
pub struct Some(Any);

impl Evaluate for Some {
    fn evaluate(
        &self, _world: &mut World, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<Box<dyn PartialReflect>> {
        let inner = self.0.0.clone_value();
        let mut tuple = DynamicTuple::default();
        tuple.insert_boxed(inner);
        Ok(Box::new(DynamicEnum::new("Some", DynamicVariant::Tuple(tuple))))
    }
}
