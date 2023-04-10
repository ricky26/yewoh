use std::fmt::Formatter;

use bevy_ecs::entity::Entity;
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use serde::{Deserializer, Serialize, Serializer};
use serde::ser::SerializeSeq;
use crate::persistence::SerializeContext;

use super::DeserializeContext;
use super::EntityReference;

pub struct EntityListVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
}

impl<'a, 'w> EntityListVisitor<'a, 'w>  {
    pub fn new(ctx: &'a mut DeserializeContext<'w>) -> Self {
        Self { ctx }
    }
}

impl<'a, 'w, 'de> Visitor<'de> for EntityListVisitor<'a, 'w> {
    type Value = Vec<Entity>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "entity list")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let mut result = Vec::new();
        if let Some(hint) = seq.size_hint() {
            result.reserve(hint);
        }

        while let Some(entity_ref) = seq.next_element::<EntityReference>()? {
            result.push(self.ctx.map_entity(entity_ref));
        }

        Ok(result)
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for EntityListVisitor<'a, 'w> {
    type Value = Vec<Entity>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_seq(self)
    }
}

pub struct EntityListSerializer<'a> {
    ctx: &'a SerializeContext,
    entities: &'a [Entity],
}

impl<'a> EntityListSerializer<'a> {
    pub fn new(ctx: &'a SerializeContext, entities: &'a [Entity]) -> Self {
        Self { ctx, entities }
    }
}

impl<'a> Serialize for EntityListSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(None)?;

        for entity in self.entities.iter().copied() {
            seq.serialize_element(&self.ctx.map_entity(entity))?;
        }

        seq.end()
    }
}
