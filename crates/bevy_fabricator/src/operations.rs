use bevy::prelude::*;
use bevy::reflect::ReflectRef;

use crate::any::Any;
use crate::traits::{Apply, Context, Evaluate, ReflectApply, ReflectEvaluate};
use crate::Fabricator;

#[derive(Clone, Default, Reflect)]
#[reflect(Default, Evaluate)]
pub struct Spawn;

impl Evaluate for Spawn {
    fn evaluate(&self, ctx: &mut Context) -> anyhow::Result<Box<dyn PartialReflect>> {
        Ok(Box::new(ctx.world.spawn_empty().id()))
    }
}

#[derive(Clone, Reflect)]
#[reflect(from_reflect = false, FromReflect, Apply)]
pub struct Fabricate {
    pub fabricator: Fabricator,
    pub parameters: Any,
}

impl FromReflect for Fabricate {
    fn from_reflect(reflect: &dyn PartialReflect) -> Option<Self> {
        match reflect.reflect_ref() {
            ReflectRef::Struct(reflect) => {
                let fabricator = Fabricator::from_reflect(reflect.field("fabricator")?)?;
                let parameters = reflect.field("parameters")
                    .and_then(Any::from_reflect)
                    .unwrap_or_else(Any::default);
                Some(Fabricate { fabricator, parameters })
            }
            ReflectRef::TupleStruct(reflect) => {
                let fabricator = Fabricator::from_reflect(reflect.field(0)?)?;
                let parameters = reflect.field(1)
                    .and_then(Any::from_reflect)
                    .unwrap_or_else(Any::default);
                Some(Fabricate { fabricator, parameters })
            }
            ReflectRef::Tuple(reflect) => {
                let fabricator = Fabricator::from_reflect(reflect.field(0)?)?;
                let parameters = reflect.field(1)
                    .and_then(Any::from_reflect)
                    .unwrap_or_else(Any::default);
                Some(Fabricate { fabricator, parameters })
            }
            _ => None,
        }
    }
}

impl Apply for Fabricate {
    fn apply(&self, ctx: &mut Context, entity: Entity) -> anyhow::Result<()> {
        let fabricated = self.fabricator.fabricate(
            self.parameters.0.as_ref(), ctx.world, entity)?;
        ctx.world.entity_mut(entity).insert(fabricated);
        Ok(())
    }
}
