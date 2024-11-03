use std::time::Duration;

use bevy::prelude::*;
use serde::Deserialize;
use yewoh_server::world::characters::Animation;
use yewoh_server::world::entity::MapPosition;

use crate::persistence::SerializationSetupExt;

pub mod prefabs;

pub mod player;

pub mod persistence;

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct Invulnerable;

#[derive(Debug, Clone, Event)]
pub struct DamageDealt {
    pub target: Entity,
    pub source: Entity,
    pub damage: u16,
    pub location: MapPosition,
}

#[derive(Debug, Clone, Event)]
pub struct CharacterDied {
    pub character: Entity,
}

#[derive(Debug, Default, Clone, Component)]
pub struct Corpse;

#[derive(Debug, Clone, Event)]
pub struct CorpseSpawned {
    pub character: Entity,
    pub corpse: Entity,
}

#[derive(Debug, Clone, Component)]
pub struct HitAnimation {
    pub hit_animation: Animation,
}

#[derive(Debug, Clone, Default, Reflect, Component, Deserialize)]
#[reflect(Component, Deserialize)]
pub struct MeleeWeapon {
    pub damage: u16,
    #[serde(with = "humantime_serde")]
    pub delay: Duration,
    pub range: i32,
    pub swing_animation: Animation,
}

#[derive(Debug, Clone, Component)]
pub struct Unarmed {
    pub weapon: MeleeWeapon,
}

pub struct CharactersPlugin;

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<Invulnerable>()
            .register_type::<player::NewPlayerCharacter>()
            .register_type::<prefabs::CharacterPrefab>()
            .register_type::<persistence::CustomStats>()
            .add_systems(Update, (
                player::spawn_starting_items,
            ))
            .register_serializer::<persistence::CustomStatsSerializer>();
    }
}
