use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{FromWorld, World};
use bevy_ecs::query::{With, WorldQuery};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::entities::{Persistent, UniqueId};
use crate::persistence::{BundleSerializer, DeserializeContext, SerializeContext};

pub struct UniqueIdSerializer;

impl FromWorld for UniqueIdSerializer {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl BundleSerializer for UniqueIdSerializer {
    type Query = &'static UniqueId;
    type Filter = With<Persistent>;
    type Bundle = UniqueId;

    fn id() -> &'static str {
        "UniqueId"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        item.clone()
    }

    fn serialize<S: Serializer>(_ctx: &SerializeContext, s: S, bundle: &Self::Bundle) -> Result<S::Ok, S::Error> {
        bundle.serialize(s)
    }

    fn deserialize<'de, D: Deserializer<'de>>(ctx: &mut DeserializeContext, d: D, entity: Entity) -> Result<(), D::Error> {
        let id = UniqueId::deserialize(d)?;
        log::debug!("deserialized id {} for {entity:?}", &id.id);
        let mut entity = ctx.world_mut()
            .entity_mut(entity);
        entity.insert(id);
        log::debug!("components = {}", entity.archetype().components().count());
        Ok(())
    }
}
