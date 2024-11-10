use bevy::prelude::*;
use yewoh_server::world::characters::CharacterBodyType;
use yewoh_server::world::entity::{ContainedPosition, EquipmentSlot, EquippedPosition, Hue, MapPosition};
use yewoh_server::world::items::ItemQuantity;
use yewoh_server::world::ServerSet;

use crate::activities::butchering::ButcheringPrefab;
use crate::activities::loot::LootPrefab;
use crate::data::prefabs::{PrefabLibraryEntityExt, PrefabLibraryWorldExt};
use crate::entities::persistence::CustomHue;
use crate::entities::Persistent;
use crate::entities::position::PositionExt;
use crate::items::persistence::CustomQuantity;

#[derive(Debug, Default, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct CorpsePrefab(pub String);

#[derive(Debug, Clone, Event)]
pub struct OnCharacterDeath {
    pub character: Entity,
}

#[derive(Debug, Default, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Corpse;

#[derive(Debug, Default, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct CorpseEquipment {
    pub slot: EquipmentSlot,
}

#[derive(Debug, Clone, Event)]
pub struct OnSpawnCorpse {
    pub character: Entity,
    pub corpse: Entity,
}

pub fn remove_dead_characters(mut commands: Commands, mut events: EventReader<OnCharacterDeath>) {
    for event in events.read() {
        commands.entity(event.character).despawn_recursive();
    }
}

pub fn spawn_corpses(
    mut commands: Commands,
    mut died_events: EventReader<OnCharacterDeath>,
    mut corpse_events: EventWriter<OnSpawnCorpse>,
    characters: Query<(
        &CharacterBodyType,
        &Hue,
        &MapPosition,
        Option<&Children>,
        &CorpsePrefab,
        Option<&LootPrefab>,
        Option<&ButcheringPrefab>,
        Has<Persistent>,
    )>,
    equipment: Query<&EquippedPosition>,
) {
    for event in died_events.read() {
        let Ok((body_type, hue, map_position, children, prefab, loot, butchering, is_persistent)) = characters.get(event.character) else {
            continue;
        };

        let mut corpse = commands
            .fabricate_prefab(&prefab.0);

        corpse.insert((
            *map_position,
            CustomQuantity,
            CustomHue,
            ItemQuantity(**body_type),
            Hue(**hue),
            Persistent,
        ));

        if is_persistent {
            corpse.insert(Persistent);
        }

        if let Some(loot) = loot {
            corpse.fabricate_insert(&loot.0);
        }

        if let Some(butchering) = butchering {
            corpse.insert(butchering.clone());
        }

        let corpse = corpse.id();
        if let Some(children) = children {
            for child_entity in children {
                let Ok(position) = equipment.get(*child_entity) else {
                    continue;
                };

                commands.entity(*child_entity)
                    .insert(CorpseEquipment {
                        slot: position.slot,
                    })
                    .move_to_container_position(corpse, ContainedPosition::default());
            }
        }

        corpse_events.send(OnSpawnCorpse {
            character: event.character,
            corpse,
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<Corpse>()
        .register_type::<CorpseEquipment>()
        .register_type::<CorpsePrefab>()
        .add_event::<OnCharacterDeath>()
        .add_event::<OnSpawnCorpse>()
        .add_systems(Update, (
            spawn_corpses,
        ))
        .add_systems(Last, (
            remove_dead_characters.in_set(ServerSet::DestroyEntities),
        ));
}
