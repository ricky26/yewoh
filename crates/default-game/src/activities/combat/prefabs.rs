use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use serde::Deserialize;

use crate::characters::{MeleeWeapon, Unarmed};
use crate::data::prefab::{FromPrefabTemplate, PrefabBundle};

#[derive(Clone, Deserialize)]
pub struct MeleeWeaponPrefab {
    #[serde(flatten)]
    weapon: MeleeWeapon,
}

impl FromPrefabTemplate for MeleeWeaponPrefab {
    type Template = MeleeWeaponPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for MeleeWeaponPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        world.entity_mut(entity)
            .insert(self.weapon.clone());
    }
}

#[derive(Clone, Deserialize)]
pub struct UnarmedPrefab {
    #[serde(flatten)]
    weapon: MeleeWeapon,
}

impl FromPrefabTemplate for UnarmedPrefab {
    type Template = UnarmedPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for UnarmedPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        world.entity_mut(entity)
            .insert(Unarmed {
                weapon: self.weapon.clone(),
            });
    }
}
