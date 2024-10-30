use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::world::{FromWorld, World};
use crate::data::prefabs::{PrefabLibraryEntityExt, PrefabLibraryRequest};
use crate::entities::{Persistent, PrefabInstance};

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

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity)
            .fabricate_from_library(PrefabLibraryRequest {
                prefab_name: bundle.prefab_name,
                parameters: bundle.parameters,
            });
    }
}
