use std::collections::VecDeque;

use bevy_ecs::entity::Entity;
use serde::{Serialize, Serializer};
use serde::ser::{Error as SError, SerializeSeq, SerializeTuple};

use super::{BundleSerializer, SerializeContext, SerializedBuffer};

struct BufferBundleSerializer<'a> {
    ctx: &'a SerializeContext,
    buffer: &'a SerializedBuffer,
}

impl<'a> Serialize for BufferBundleSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut tuple = serializer.serialize_tuple(2)?;
        tuple.serialize_element(&self.buffer.serializer_id)?;
        tuple.serialize_element(&BufferValuesSerializer { ctx: self.ctx, buffer: &self.buffer })?;
        tuple.end()
    }
}

pub(crate) struct BufferBundlesSerializer<'a> {
    ctx: &'a SerializeContext,
    buffers: &'a VecDeque<SerializedBuffer>,
}

impl<'a> BufferBundlesSerializer<'a> {
    pub fn new(ctx: &'a SerializeContext, buffers: &'a VecDeque<SerializedBuffer>) -> Self {
        Self { ctx, buffers }
    }
}

impl<'a> Serialize for BufferBundlesSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(self.buffers.len()))?;

        for buffer in self.buffers {
            seq.serialize_element(&BufferBundleSerializer {
                ctx: self.ctx,
                buffer,
            })?;
        }

        seq.end()
    }
}

struct BufferValueSerializer<'a, T: BundleSerializer> {
    ctx: &'a SerializeContext,
    bundle: &'a T::Bundle,
}

impl<'a, T: BundleSerializer> Serialize for BufferValueSerializer<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        T::serialize(self.ctx, serializer, self.bundle)
            .map_err(S::Error::custom)
    }
}

struct BufferValuePairSerializer<'a, T: BundleSerializer> {
    ctx: &'a SerializeContext,
    entity: Entity,
    bundle: &'a T::Bundle,
}

impl<'a, T: BundleSerializer> Serialize for BufferValuePairSerializer<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.ctx.map_entity(self.entity))?;
        seq.serialize_element(&BufferValueSerializer::<T> {
            ctx: self.ctx,
            bundle: self.bundle,
        })?;
        seq.end()
    }
}

struct BufferValuesSerializer<'a> {
    ctx: &'a SerializeContext,
    buffer: &'a SerializedBuffer,
}

impl<'a> Serialize for BufferValuesSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut serializer = Some(serializer);
        let mut result = None;
        (self.buffer.serialize)(self.ctx, &mut |s| {
            result = Some(erased_serde::serialize(s, serializer.take().unwrap()));
        });
        result.unwrap()
    }
}

pub(crate) struct BufferSerializer<'a, T: BundleSerializer> {
    ctx: &'a SerializeContext,
    bundles: &'a [(Entity, T::Bundle)],
}

impl<'a, T: BundleSerializer> BufferSerializer<'a, T> {
    pub fn new(ctx: &'a SerializeContext, bundles: &'a [(Entity, T::Bundle)]) -> Self {
        Self { ctx, bundles }
    }
}

impl<'a, T: BundleSerializer> Serialize for BufferSerializer<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(self.bundles.len()))?;
        for (entity, bundle) in self.bundles {
            seq.serialize_element(&BufferValuePairSerializer::<T> {
                ctx: self.ctx,
                entity: entity.clone(),
                bundle,
            })?;
        }
        seq.end()
    }
}
