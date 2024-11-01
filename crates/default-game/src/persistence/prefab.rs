use bevy::prelude::*;

use crate::data::prefabs::{PrefabLibraryEntityExt};
use crate::entities::{Persistent, PrefabInstance};

use super::BundleSerializer;

#[derive(Default)]
pub struct PrefabSerializer;

impl BundleSerializer for PrefabSerializer {
    type Query = &'static PrefabInstance;
    type Filter = With<Persistent>;
    type Bundle = String;

    fn id() -> &'static str {
        "Prefab"
    }

    fn priority() -> i32 {
        -1000
    }

    fn extract(item: &PrefabInstance) -> Self::Bundle {
        item.prefab_name.clone()
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity)
            .fabricate_prefab(bundle)
            .insert(Persistent);
    }
}
