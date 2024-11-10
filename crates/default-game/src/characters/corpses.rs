use bevy::prelude::*;
use yewoh_server::world::characters::CharacterBodyType;
use yewoh_server::world::entity::{Hue, MapPosition};
use yewoh_server::world::items::ItemQuantity;

use crate::data::prefabs::PrefabLibraryWorldExt;
use crate::entities::Persistent;

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
    characters: Query<(&CharacterBodyType, &Hue, &MapPosition, &CorpsePrefab, Has<Persistent>)>,
) {
    for event in died_events.read() {
        let Ok((body_type, hue, map_position, prefab, is_persistent)) = characters.get(event.character) else {
            continue;
        };

        let corpse = commands
            .fabricate_prefab(&prefab.0)
            .insert((
                *map_position,
                ItemQuantity(**body_type),
                Hue(**hue),
                Corpse,
            ))
            .id();
        corpse_events.send(OnSpawnCorpse {
            character: event.character,
            corpse,
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<Corpse>()
        .register_type::<CorpsePrefab>()
        .add_event::<OnCharacterDeath>()
        .add_event::<OnSpawnCorpse>()
        .add_systems(Update, (
            (
                spawn_corpses,
                remove_dead_characters,
            ).chain(),
        ));
}
