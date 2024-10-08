use bevy::ecs::entity::Entity;
use bevy::ecs::world::{FromWorld, World};
use bevy::ecs::query::With;
use serde::{Deserialize, Deserializer, Serializer};
use serde::de::Error;

use crate::data::prefab::{PrefabCollection, PrefabCommandsExt};
use crate::entities::{Persistent, PrefabInstance};
use crate::persistence::{DeserializeContext, SerializeContext};

use super::BundleSerializer;

pub struct PrefabSerializer;

impl FromWorld for PrefabSerializer {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl BundleSerializer for PrefabSerializer {
    type Query = &'static PrefabInstance;
    type Filter = With<Persistent>;
    type Bundle = PrefabInstance;

    fn id() -> &'static str {
        "Prefab"
    }

    fn priority() -> i32 {
        -1000
    }

    fn extract(item: &PrefabInstance) -> Self::Bundle {
        item.clone()
    }

    fn serialize<S: Serializer>(_ctx: &SerializeContext, s: S, bundle: &Self::Bundle) -> Result<S::Ok, S::Error> {
        s.serialize_str(&bundle.prefab_name)
    }

    fn deserialize<'de, D: Deserializer<'de>>(ctx: &mut DeserializeContext, d: D, entity: Entity) -> Result<(), D::Error> {
        let prefab_name = String::deserialize(d)?;
        if let Some(prefab) = ctx.world.resource::<PrefabCollection>().get(&prefab_name).cloned() {
            ctx.world.entity_mut(entity)
                .insert_prefab(prefab)
                .insert((
                    PrefabInstance { prefab_name: prefab_name.into() },
                    Persistent,
                ));
            Ok(())
        } else {
            Err(D::Error::custom(format!("Unable to deserialize prefab {}", &prefab_name)))
        }
    }
}
