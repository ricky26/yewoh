use bevy::prelude::*;

use crate::traits::{Context, Evaluate, ReflectEvaluate};

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

impl Clone for Any {
    fn clone(&self) -> Self {
        Any::from_reflect(self.0.as_ref()).unwrap()
    }
}

impl Default for Any {
    fn default() -> Self {
        Any(Box::new(()))
    }
}

impl Evaluate for Any {
    fn evaluate(&self, _ctx: &mut Context) -> anyhow::Result<Box<dyn PartialReflect>> {
        let cloned = self.0.clone_value();
        Ok(cloned)
    }
}

