use std::time::Duration;

use bevy::prelude::*;
use serde::Deserialize;

use yewoh_server::world::entity::MapPosition;
use yewoh_server::world::ServerSet;

use crate::characters::animation::AnimationStartedEvent;
use crate::characters::prefabs::CharacterPrefab;
use crate::persistence::SerializationSetupExt;

pub mod prefabs;

pub mod animation;

pub mod player;

pub mod persistence;

#[derive(Debug, Default, Clone, Component)]
pub struct Alive;

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

#[derive(Debug, Clone, Default, Reflect, Deserialize)]
pub struct AnimationDefinition {
    pub animation_id: u16,
    pub frame_count: u16,
    #[serde(default)]
    pub repeat_count: u16,
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub speed: u8,
}

#[derive(Debug, Clone, Default, Reflect, Deserialize)]
pub struct PredefinedAnimation {
    pub kind: u16,
    pub action: u16,
    #[serde(default)]
    pub variant: u8,
}

#[derive(Debug, Clone, Reflect, Deserialize)]
#[serde(untagged)]
pub enum Animation {
    Inline(AnimationDefinition),
    Predefined(PredefinedAnimation),
}

impl Default for Animation {
    fn default() -> Self {
        Animation::Inline(Default::default())
    }
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
            .register_type::<CharacterPrefab>()
            .register_type::<player::NewPlayerCharacter>()
            .register_type::<persistence::CustomStats>()
            .add_event::<AnimationStartedEvent>()
            .add_systems(Update, (
                player::spawn_starting_items,
            ))
            .add_systems(Last, (
                animation::send_animations.in_set(ServerSet::Send),
            ))
            .register_serializer::<persistence::CustomStatsSerializer>();
    }
}
