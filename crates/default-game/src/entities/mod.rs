use bevy::prelude::*;
use bevy::reflect::serde::{DeserializeWithRegistry, ReflectDeserializeWithRegistry, ReflectDeserializer, ReflectSerializeWithRegistry, ReflectSerializer, SerializeWithRegistry};
use bevy::reflect::{Reflect, ReflectDeserialize, ReflectSerialize, TypeRegistry};
use rand::{thread_rng, RngCore};
use serde::de::{Error, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Formatter;
use std::sync::Arc;
use uuid::{Bytes, Uuid};

use crate::persistence::SerializationSetupExt;

pub mod persistence;

pub mod position;

pub mod prefabs;

#[derive(Debug, Clone, Copy, Default, Reflect, Component)]
#[reflect(Component)]
pub struct Persistent;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component, SerializeWithRegistry, DeserializeWithRegistry)]
pub struct PrefabInstance {
    pub prefab_name: String,
    pub parameters: Arc<dyn PartialReflect>,
}

impl SerializeWithRegistry for PrefabInstance {
    fn serialize<S>(&self, serializer: S, registry: &TypeRegistry) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("PrefabInstance", 2)?;
        s.serialize_field("prefab_name", &self.prefab_name)?;
        s.serialize_field("parameters", &ReflectSerializer::new(&*self.parameters, registry))?;
        s.end()
    }
}

impl<'de> DeserializeWithRegistry<'de> for PrefabInstance {
    fn deserialize<D>(deserializer: D, registry: &TypeRegistry) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PrefabInstanceVisitor<'a> {
            registry: &'a TypeRegistry,
        }

        impl<'de> Visitor<'de> for PrefabInstanceVisitor<'_> {
            type Value = PrefabInstance;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a prefab instance")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut prefab_name = None;
                let mut parameters = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "prefab_name" => prefab_name = Some(map.next_value::<String>()?),
                        "parameters" => parameters =
                            Some(map.next_value_seed(ReflectDeserializer::new(self.registry))?),
                        _ => return Err(A::Error::unknown_field(
                            &key, &["prefab_name", "parameters"])),
                    }

                }

                let prefab_name = prefab_name
                    .ok_or_else(|| A::Error::missing_field("prefab_name"))?;
                let parameters = parameters
                    .ok_or_else(|| A::Error::missing_field("parameters"))?;
                Ok(PrefabInstance { prefab_name, parameters: parameters.into() })
            }
        }

        deserializer.deserialize_struct(
            "PrefabInstance",
            &["prefab_name", "parameters"],
            PrefabInstanceVisitor { registry })
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct UniqueId {
    pub id: Uuid,
}

impl Serialize for UniqueId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.id.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UniqueId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(UniqueId { id: Uuid::deserialize(deserializer)? })
    }
}

impl UniqueId {
    pub fn new() -> UniqueId {
        UniqueId { id: new_uuid() }
    }
}

pub fn new_uuid() -> Uuid {
    let mut bytes = Bytes::default();
    thread_rng().fill_bytes(&mut bytes[..]);
    Uuid::from_bytes(bytes)
}

#[derive()]
pub struct EntitiesPlugin;

impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<UniqueId>()
            .register_type::<Persistent>()
            .register_type::<prefabs::AtMapPosition>()
            .register_type::<prefabs::EquippedBy>()
            .register_type::<prefabs::ContainedBy>()
            .register_serializer::<persistence::UniqueIdSerializer>();
    }
}
