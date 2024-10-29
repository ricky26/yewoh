use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::reflect::{Reflect, std_traits::ReflectDefault};
use serde::Deserialize;
use bevy_fabricator::Fabricated;
use bevy_fabricator::traits::{Apply, ReflectApply};
use yewoh::Notoriety;
use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::entity::{Character, CharacterEquipped, EquippedBy, Flags, Location, Notorious, Stats};

use crate::activities::CurrentActivity;
use crate::characters::{Alive, Animation, HitAnimation};
use crate::data::prefab::{FromPrefabTemplate, Prefab, PrefabBundle};

#[derive(Clone, Reflect, Deserialize)]
pub struct EquipmentPrefab {
    #[reflect(remote = yewoh_server::remote_reflect::EquipmentSlotRemote)]
    pub slot: EquipmentSlot,
    #[serde(flatten)]
    pub prefab: Prefab,
}

#[derive(Default, Clone, Reflect, Deserialize)]
#[reflect(Default, Apply)]
#[serde(default)]
pub struct CharacterPrefab {
    pub name: String,
    pub body_type: u16,
    pub hue: u16,
    #[reflect(remote = yewoh_server::remote_reflect::NotorietyRemote)]
    pub notoriety: Notoriety,
    pub equipment: Vec<EquipmentPrefab>,
    pub hit_animation: Option<Animation>,
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
                entity: child_entity,
                slot: child.slot,
            });
        }

        let mut commands = world.entity_mut(entity);
        commands
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
                CurrentActivity::Idle,
                Alive,
            ));

        if let Some(hit_animation) = self.hit_animation.clone() {
            commands.insert(HitAnimation { hit_animation });
        }
    }
}

impl Apply for CharacterPrefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        self.write(world, entity);
        Ok(())
    }
}
