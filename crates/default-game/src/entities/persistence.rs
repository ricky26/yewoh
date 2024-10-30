use bevy::ecs::query::{With, WorldQuery};
use bevy::ecs::world::{FromWorld, World};
use bevy::prelude::Entity;
use crate::entities::{Persistent, UniqueId};
use crate::persistence::BundleSerializer;

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

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity).insert(bundle);
    }
}
