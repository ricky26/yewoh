use std::fmt::Formatter;
use std::marker::PhantomData;

use bevy::ecs::entity::Entity;
use bevy::reflect::{FromReflect, PartialReflect, TypePath};
use bevy::reflect::serde::TypedReflectDeserializer;
use serde::de::{DeserializeSeed, Error as DError, MapAccess, SeqAccess, Visitor};
use serde::Deserializer;

use super::{BundleSerializer, BundleSerializers, DeserializeContext};

struct BundleVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
    deserializers: &'a BundleSerializers,
}

impl<'de> Visitor<'de> for BundleVisitor<'_, '_> {
    type Value = (String, Box<dyn PartialReflect>);

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "bundle list")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        struct Thunk<'a, 'w> {
            ctx: &'a mut DeserializeContext<'w>,
            deserializers: &'a BundleSerializers,
            id: String,
        }

        impl<'de> DeserializeSeed<'de> for Thunk<'_, '_> {
            type Value = (String, Box<dyn PartialReflect>);

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
                let values = self.deserializers.deserialize_bundle_values(self.ctx, &self.id, deserializer)?;
                Ok((self.id, values))
            }
        }

        let id = seq.next_element::<String>()?
            .ok_or_else(|| A::Error::invalid_length(0, &"two-element tuple"))?;
        let bundles = seq.next_element_seed(Thunk {
            ctx: self.ctx,
            deserializers: self.deserializers,
            id,
        })?.ok_or_else(|| A::Error::invalid_length(0, &"two-element tuple"))?;
        Ok(bundles)
    }
}

impl<'de> DeserializeSeed<'de> for BundleVisitor<'_, '_> {
    type Value = (String, Box<dyn PartialReflect>);

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_seq(self)
    }
}

struct BundlesVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
    deserializers: &'a BundleSerializers,
}

impl<'de> Visitor<'de> for BundlesVisitor<'_, '_> {
    type Value = Vec<(String, Box<dyn PartialReflect>)>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "bundle list")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let mut bundles = Vec::new();

        if let Some(hint) = seq.size_hint() {
            bundles.reserve(hint);
        }

        while let Some(item) = seq.next_element_seed(BundleVisitor {
            ctx: self.ctx,
            deserializers: self.deserializers,
        })? {
            bundles.push(item);
        }

        Ok(bundles)
    }
}

impl<'de> DeserializeSeed<'de> for BundlesVisitor<'_, '_> {
    type Value = Vec<(String, Box<dyn PartialReflect>)>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_seq(self)
    }
}

pub(crate) struct WorldVisitor<'a, 'w> {
    pub(crate) ctx: &'a mut DeserializeContext<'w>,
    pub(crate) deserializers: &'a BundleSerializers,
}

impl<'de> Visitor<'de> for WorldVisitor<'_, '_> {
    type Value = Vec<(String, Box<dyn PartialReflect>)>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "bundle list")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        let mut bundles = Vec::new();

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "bundles" => {
                    bundles = map.next_value_seed(BundlesVisitor {
                        ctx: self.ctx,
                        deserializers: self.deserializers,
                    })?;
                }
                name => return Err(A::Error::unknown_field(name, &["bundles"])),
            }
        }

        Ok(bundles)
    }
}

impl<'de> DeserializeSeed<'de> for WorldVisitor<'_, '_> {
    type Value = Vec<(String, Box<dyn PartialReflect>)>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_struct("World", &["bundles"], self)
    }
}

struct BundleValuePairVisitor<'a, 'w, T: BundleSerializer> {
    ctx: &'a mut DeserializeContext<'w>,
    _phantom: PhantomData<T>,
}

impl<'de, T: BundleSerializer> Visitor<'de> for BundleValuePairVisitor<'_, '_, T> {
    type Value = (Entity, T::Bundle);

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "a bundle tuple")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let entity = seq.next_element::<Entity>()?
            .ok_or_else(|| A::Error::custom("missing entity ID"))?;
        let bundle = seq.next_element_seed(TypedReflectDeserializer::of::<T::Bundle>(self.ctx.type_registry))?
            .ok_or_else(|| A::Error::custom("missing bundle"))?;
        let bundle = T::Bundle::from_reflect(&*bundle)
            .ok_or_else(|| {
                let ty_path = T::Bundle::type_path();
                A::Error::custom(format!("could not parse bundle {ty_path}"))
            })?;
        Ok((entity, bundle))
    }
}

impl<'de, T: BundleSerializer> DeserializeSeed<'de> for BundleValuePairVisitor<'_, '_, T> {
    type Value = (Entity, T::Bundle);

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_tuple(2, self)
    }
}

pub(crate) struct BundleValuesVisitor<'a, 'w, T: BundleSerializer> {
    ctx: &'a mut DeserializeContext<'w>,
    _phantom: PhantomData<T>,
}

impl<'a, 'w, T: BundleSerializer> BundleValuesVisitor<'a, 'w, T> {
    pub fn new(ctx: &'a mut DeserializeContext<'w>) -> Self {
        Self {
            ctx,
            _phantom: PhantomData,
        }
    }
}

impl<'de, T: BundleSerializer> Visitor<'de> for BundleValuesVisitor<'_, '_, T> {
    type Value = Box<dyn PartialReflect>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "list of bundle values")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let mut pairs = Vec::new();

        if let Some(hint) = seq.size_hint() {
            pairs.reserve(hint);
        }

        while let Some(item) = seq.next_element_seed(BundleValuePairVisitor {
            ctx: self.ctx,
            _phantom: self._phantom,
        })? {
            pairs.push(item);
        }

        Ok(Box::new(pairs))
    }
}

impl<'de, T: BundleSerializer> DeserializeSeed<'de> for BundleValuesVisitor<'_, '_, T> {
    type Value = Box<dyn PartialReflect>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_seq(self)
    }
}

