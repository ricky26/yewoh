use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use serde::Deserialize;

use yewoh::Notoriety;
use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::entity::{Character, CharacterEquipped, EquippedBy, Flags, Location, Notorious, Stats};

use crate::characters::Alive;
use crate::data::prefab::{FromPrefabTemplate, Prefab, PrefabBundle};

#[derive(Clone, Deserialize)]
pub struct EquipmentPrefab {
    pub slot: EquipmentSlot,
    #[serde(flatten)]
    pub prefab: Prefab,
}

#[derive(Default, Clone, Deserialize)]
#[serde(default)]
pub struct CharacterPrefab {
    pub name: String,
    pub body_type: u16,
    pub hue: u16,
    pub notoriety: Notoriety,
    pub equipment: Vec<EquipmentPrefab>,
}

impl FromPrefabTemplate for CharacterPrefab {
    type Template = CharacterPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for CharacterPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        let mut equipment = Vec::with_capacity(self.equipment.len());

        for child in &self.equipment {
            let child_entity = world.spawn_empty()
                .insert((
                    EquippedBy {
                        parent: entity,
                        slot: child.slot,
                    },
                ))
                .id();
            child.prefab.write(world, child_entity);
            equipment.push(CharacterEquipped {
                equipment: child_entity,
                slot: child.slot,
            });
        }

        world.entity_mut(entity)
            .insert((
                Flags::default(),
                Location::default(),
                Notorious(self.notoriety),
                Character {
                    body_type: self.body_type,
                    hue: self.hue,
                    equipment,
                },
                Stats {
                    name: self.name.to_string(),
                    hp: 100,
                    max_hp: 100,
                    ..Default::default()
                },
                Alive,
            ));
    }
}
