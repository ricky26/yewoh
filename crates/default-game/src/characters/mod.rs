use std::time::Duration;

use bevy::app::{App, Last, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::schedule::IntoSystemConfigs;
use serde::Deserialize;

use crate::characters::animation::AnimationStartedEvent;
use yewoh_server::world::entity::Location;
use yewoh_server::world::ServerSet;

use crate::characters::prefabs::CharacterPrefab;
use crate::data::prefab::PrefabAppExt;
use crate::persistence::SerializationSetupExt;

pub mod prefabs;

pub mod animation;

mod persistence;

#[derive(Debug, Default, Clone, Component)]
pub struct Alive;

#[derive(Debug, Clone, Event)]
pub struct DamageDealt {
    pub target: Entity,
    pub source: Entity,
    pub damage: u16,
    pub location: Location,
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct PredefinedAnimation {
    pub kind: u16,
    pub action: u16,
    #[serde(default)]
    pub variant: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Animation {
    Inline(AnimationDefinition),
    Predefined(PredefinedAnimation),
}

#[derive(Debug, Clone, Component)]
pub struct HitAnimation {
    pub hit_animation: Animation,
}

#[derive(Debug, Clone, Component, Deserialize)]
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
            .init_prefab_bundle::<CharacterPrefab>("character")
            .add_event::<AnimationStartedEvent>()
            .add_systems(Last, (
                animation::send_animations.in_set(ServerSet::Send),
            ))
            .register_serializer::<persistence::CharacterSerializer>();
    }
}
