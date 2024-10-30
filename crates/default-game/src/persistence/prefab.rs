use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::world::{FromWorld, World};

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
        world.entity_mut(entity).insert(bundle);

        /*
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

         */
    }
}
