use bevy_ecs::system::EntityCommands;
use serde::Deserialize;

use yewoh::Notoriety;
use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::entity::{Character, CharacterEquipped, EquippedBy, Flags, MapPosition, Notorious, Stats};

use crate::characters::Alive;
use crate::data::prefab::{FromPrefabTemplate, Prefab, PrefabBundle, PrefabEntityReference};

#[derive(Clone, Deserialize)]
pub struct EquipmentPrefab {
    pub slot: EquipmentSlot,
    pub entity: PrefabEntityReference,
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
    fn spawn(&self, prefab: &Prefab, commands: &mut EntityCommands<'_, '_, '_>) {
        let parent = commands.id();
        let mut equipment = Vec::with_capacity(self.equipment.len());

        for child in &self.equipment {
            let commands = commands.commands();
            let mut child_commands = commands.spawn_empty();
            child_commands
                .insert((
                    EquippedBy {
                        parent,
                        slot: child.slot,
                    },
                ));
            prefab.insert_child(&child.entity, &mut child_commands);
            equipment.push(CharacterEquipped {
                equipment: child_commands.id(),
                slot: child.slot,
            });
        }

        commands
            .insert((
                Flags::default(),
                MapPosition::default(),
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
