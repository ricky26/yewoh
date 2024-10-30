use crate::persistence::SerializationSetupExt;
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::reflect::ReflectComponent;
use bevy::reflect::{Reflect, ReflectSerialize, ReflectDeserialize};
use rand::{thread_rng, RngCore};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::{Bytes, Uuid};

pub mod persistence;

#[derive(Debug, Clone, Copy, Default, Component)]
pub struct Persistent;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct PrefabInstance {
    pub prefab_name: String,
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(opaque, Serialize, Deserialize)]
pub struct UniqueId {
    pub id: Uuid,
}

impl Serialize for UniqueId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.id.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UniqueId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
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
            .register_serializer::<persistence::UniqueIdSerializer>();
    }
}
