use bevy::prelude::*;

use crate::traits::{Evaluate, ReflectEvaluate};

#[derive(Reflect)]
#[reflect(from_reflect = false, FromReflect, Evaluate)]
pub struct Any(#[reflect(ignore)] pub Box<dyn PartialReflect>);

impl FromReflect for Any {
    fn from_reflect(reflect: &dyn PartialReflect) -> Option<Self> {
        if let Some(existing) = reflect.try_downcast_ref::<Any>() {
            Some(Any(existing.0.clone_value()))
        } else {
            Some(Any(reflect.clone_value()))
        }
    }
}

impl Evaluate for Any {
    fn evaluate(&self, _world: &mut World) -> anyhow::Result<Box<dyn PartialReflect>> {
        let cloned = self.0.clone_value();
        Ok(cloned)
    }
}

