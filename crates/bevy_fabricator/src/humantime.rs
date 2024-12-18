use bevy::prelude::*;
use crate::traits::{Context, Evaluate, ReflectEvaluate};

#[derive(Clone, Default, Reflect)]
#[reflect(Default, Evaluate)]
pub struct HumanDuration(pub String);

impl Evaluate for HumanDuration {
    fn evaluate(&self, _ctx: &mut Context) -> anyhow::Result<Box<dyn PartialReflect>> {
        Ok(Box::new(humantime::parse_duration(&self.0)?))
    }
}
