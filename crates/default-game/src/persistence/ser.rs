use std::collections::VecDeque;

use bevy::ecs::entity::Entity;
use bevy::reflect::serde::TypedReflectSerializer;
use serde::{Serialize, Serializer};
use serde::ser::{SerializeSeq, SerializeTuple};

use super::{BundleSerializer, SerializeContext, SerializedBuffer};

struct BufferBundleSerializer<'a> {
    buffer: &'a SerializedBuffer,
}

impl<'a> Serialize for BufferBundleSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut tuple = serializer.serialize_tuple(2)?;
        tuple.serialize_element(&self.buffer.serializer_id)?;
        tuple.serialize_element(&BufferValuesSerializer { buffer: self.buffer })?;
        tuple.end()
    }
}

pub(crate) struct BufferBundlesSerializer<'a> {
    buffers: &'a VecDeque<SerializedBuffer>,
}

impl<'a> BufferBundlesSerializer<'a> {
    pub fn new(buffers: &'a VecDeque<SerializedBuffer>) -> Self {
        Self { buffers }
    }
}

impl<'a> Serialize for BufferBundlesSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(self.buffers.len()))?;

        for buffer in self.buffers {
            seq.serialize_element(&BufferBundleSerializer {
                buffer,
            })?;
        }

        seq.end()
    }
}

struct BufferValuePairSerializer<'a, 'b, T: BundleSerializer> {
    ctx: &'a SerializeContext<'b>,
    entity: Entity,
    bundle: &'a T::Bundle,
}

impl<'a, 'b, T: BundleSerializer> Serialize for BufferValuePairSerializer<'a, 'b, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.entity)?;
        seq.serialize_element(&TypedReflectSerializer::new(self.bundle, &self.ctx.type_registry))?;
        seq.end()
    }
}

struct BufferValuesSerializer<'a> {
    buffer: &'a SerializedBuffer,
}

impl<'a> Serialize for BufferValuesSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut serializer = Some(serializer);
        let mut result = None;
        (self.buffer.serialize)(&mut |s| {
            result = Some(erased_serde::serialize(s, serializer.take().unwrap()));
        });
        result.unwrap()
    }
}

pub(crate) struct BufferSerializer<'a, 'b, T: BundleSerializer> {
    ctx: &'a SerializeContext<'b>,
    bundles: &'a [(Entity, T::Bundle)],
}

impl<'a, 'b, T: BundleSerializer> BufferSerializer<'a, 'b, T> {
    pub fn new(ctx: &'a SerializeContext<'b>, bundles: &'a [(Entity, T::Bundle)]) -> Self {
        Self { ctx, bundles }
    }
}

impl<'a, 'b, T: BundleSerializer> Serialize for BufferSerializer<'a, 'b, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(self.bundles.len()))?;
        for (entity, bundle) in self.bundles {
            seq.serialize_element(&BufferValuePairSerializer::<T> {
                ctx: self.ctx,
                entity: *entity,
                bundle,
            })?;
        }
        seq.end()
    }
}
