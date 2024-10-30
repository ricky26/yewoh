use bevy::prelude::*;
use bevy::reflect::ReflectRef;
use crate::any::Any;
use crate::traits::{Apply, Evaluate, ReflectApply, ReflectEvaluate};
use crate::{Fabricated, Fabricator};

#[derive(Clone, Default, Reflect)]
#[reflect(Default, Evaluate)]
pub struct Spawn;

impl Evaluate for Spawn {
    fn evaluate(
        &self, world: &mut World, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<Box<dyn PartialReflect>> {
        Ok(Box::new(world.spawn_empty().id()))
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
                    .and_then(|r| Any::from_reflect(r))
                    .unwrap_or_else(|| Any::default());
                Some(Fabricate { fabricator, parameters })
            }
            ReflectRef::TupleStruct(reflect) => {
                let fabricator = Fabricator::from_reflect(reflect.field(0)?)?;
                let parameters = reflect.field(1)
                    .and_then(|r| Any::from_reflect(r))
                    .unwrap_or_else(|| Any::default());
                Some(Fabricate { fabricator, parameters })
            }
            ReflectRef::Tuple(reflect) => {
                let fabricator = Fabricator::from_reflect(reflect.field(0)?)?;
                let parameters = reflect.field(1)
                    .and_then(|r| Any::from_reflect(r))
                    .unwrap_or_else(|| Any::default());
                Some(Fabricate { fabricator, parameters })
            }
            _ => None,
        }
    }
}

impl Apply for Fabricate {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        let fabricated = self.fabricator.fabricate(self.parameters.0.as_ref(), world, entity)?;
        world.entity_mut(entity).insert(fabricated);
        Ok(())
    }
}
