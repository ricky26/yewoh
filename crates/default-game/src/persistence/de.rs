use std::fmt::Formatter;
use std::marker::PhantomData;

use serde::Deserializer;
use serde::de::{DeserializeSeed, Error as DError, MapAccess, SeqAccess, Visitor};

use super::{BundleSerializer, BundleSerializers, DeserializeContext};

struct BundleVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
    deserializers: &'a BundleSerializers,
}

impl<'a, 'w, 'de> Visitor<'de> for BundleVisitor<'a, 'w> {
    type Value = ();

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "bundle list")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        struct Thunk<'a, 'w> {
            ctx: &'a mut DeserializeContext<'w>,
            deserializers: &'a BundleSerializers,
            id: String,
        }

        impl<'a, 'w, 'de> DeserializeSeed<'de> for Thunk<'a, 'w> {
            type Value = ();

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
                self.deserializers.deserialize_bundle_values(self.ctx, &self.id, deserializer)
            }
        }

        let id = seq.next_element::<String>()?
            .ok_or_else(|| A::Error::invalid_length(0, &"two-element tuple"))?;
        seq.next_element_seed(Thunk { ctx: self.ctx, deserializers: self.deserializers, id })?;
        Ok(())
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for BundleVisitor<'a, 'w> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_seq(self)
    }
}

struct BundlesVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
    deserializers: &'a BundleSerializers,
}

impl<'a, 'w, 'de> Visitor<'de> for BundlesVisitor<'a, 'w> {
    type Value = ();

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "bundle list")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        while let Some(_) = seq.next_element_seed(BundleVisitor {
            ctx: self.ctx,
            deserializers: self.deserializers,
        })? {}

        Ok(())
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for BundlesVisitor<'a, 'w> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_seq(self)
    }
}

pub(crate) struct WorldVisitor<'a, 'w> {
    pub(crate) ctx: &'a mut DeserializeContext<'w>,
    pub(crate) deserializers: &'a BundleSerializers,
}

impl<'a, 'w, 'de> Visitor<'de> for WorldVisitor<'a, 'w> {
    type Value = ();

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "bundle list")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "bundles" => {
                    map.next_value_seed(BundlesVisitor {
                        ctx: self.ctx,
                        deserializers: self.deserializers,
                    })?;
                }
                name => return Err(A::Error::unknown_field(name, &["bundles"])),
            }
        }

        Ok(())
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for WorldVisitor<'a, 'w> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_struct("World", &["bundles"], self)
    }
}

struct BundleValueVisitor<'a, 'w, T: BundleSerializer> {
    ctx: &'a mut DeserializeContext<'w>,
    _phantom: PhantomData<T>,
}

impl<'a, 'w, 'de, T: BundleSerializer> DeserializeSeed<'de> for BundleValueVisitor<'a, 'w, T> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        T::deserialize(self.ctx, deserializer)
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

impl<'a, 'w, 'de, T: BundleSerializer> Visitor<'de> for BundleValuesVisitor<'a, 'w, T> {
    type Value = ();

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "list of bundle values")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        while let Some(_) = seq.next_element_seed(BundleValueVisitor {
            ctx: self.ctx,
            _phantom: self._phantom,
        })? {}

        Ok(())
    }
}

impl<'a, 'w, 'de, T: BundleSerializer> DeserializeSeed<'de> for BundleValuesVisitor<'a, 'w, T> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_seq(self)
    }
}

