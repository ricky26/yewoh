use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::reflect::Reflect;
use serde::Deserialize;
use bevy_fabricator::Fabricated;
use bevy_fabricator::traits::Apply;
use crate::characters::{MeleeWeapon, Unarmed};

#[derive(Clone, Default, Reflect, Deserialize)]
pub struct MeleeWeaponPrefab {
    #[serde(flatten)]
    weapon: MeleeWeapon,
}

impl Apply for MeleeWeaponPrefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        world.entity_mut(entity)
            .insert(self.weapon.clone());
        Ok(())
    }
}

#[derive(Clone, Reflect, Deserialize)]
pub struct UnarmedPrefab {
    #[serde(flatten)]
    weapon: MeleeWeapon,
}

impl Apply for UnarmedPrefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        world.entity_mut(entity)
            .insert(Unarmed {
                weapon: self.weapon.clone(),
            });
        Ok(())
    }
}
