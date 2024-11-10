use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use bevy::prelude::*;

use crate::traits::{Context, Evaluate, ReflectEvaluate};

#[derive(Clone, Reflect)]
#[reflect(from_reflect = false, opaque, FromReflect, Debug, Evaluate)]
pub struct Any(pub Arc<dyn PartialReflect>);

impl FromReflect for Any {
    fn from_reflect(reflect: &dyn PartialReflect) -> Option<Self> {
        if let Some(existing) = reflect.try_downcast_ref::<Any>() {
            Some(existing.clone())
        } else {
            Some(Any(reflect.clone_value().into()))
        }
    }
}

impl Default for Any {
    fn default() -> Self {
        Any(Arc::new(()))
    }
}

impl Debug for Any {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Any({:?})", self.0.as_ref())
    }
}

impl Evaluate for Any {
    fn evaluate(&self, _ctx: &mut Context) -> anyhow::Result<Box<dyn PartialReflect>> {
        let cloned = self.0.as_ref().clone_value();
        Ok(cloned)
    }
}

