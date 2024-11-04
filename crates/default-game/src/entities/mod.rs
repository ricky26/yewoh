use bevy::prelude::*;
use bevy::reflect::{ReflectDeserialize, ReflectSerialize};
use rand::{thread_rng, RngCore};
use serde::{Deserialize, Serialize};
use uuid::{Bytes, Uuid};

pub mod persistence;

pub mod position;

pub mod prefabs;

#[derive(Debug, Clone, Copy, Default, Reflect, Component)]
#[reflect(Component)]
pub struct Persistent;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct PrefabInstance {
    pub prefab_name: String,
}

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize)]
#[reflect(opaque, Component, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UniqueId {
    pub id: Uuid,
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
            .add_plugins(persistence::plugin)
            .register_type::<UniqueId>()
            .register_type::<Persistent>()
            .register_type::<prefabs::Prefab>()
            .register_type::<prefabs::AtMapPosition>()
            .register_type::<prefabs::EquippedBy>()
            .register_type::<prefabs::ContainedBy>();
    }
}
