use std::time::Duration;

use bevy_app::{App, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::IntoSystemConfig;
use serde_derive::Deserialize;

use yewoh_server::world::entity::Location;
use yewoh_server::world::ServerSet;
use crate::characters::animation::AnimationStartedEvent;

use crate::characters::prefabs::CharacterPrefab;
use crate::data::prefab::PrefabAppExt;

pub mod prefabs;

pub mod animation;

#[derive(Debug, Default, Clone, Component)]
pub struct Alive;

#[derive(Debug, Clone)]
pub struct DamageDealt {
    pub target: Entity,
    pub source: Entity,
    pub damage: u16,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub struct CharacterDied {
    pub character: Entity,
}

#[derive(Debug, Default, Clone, Component)]
pub struct Corpse;

#[derive(Debug, Clone)]
pub struct CorpseSpawned {
    pub character: Entity,
    pub corpse: Entity,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnimationDefinition {
    pub animation_id: u16,
    pub frame_count: u16,
    pub repeat_count: u16,
    pub reverse: bool,
    pub speed: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PredefinedAnimation {
    pub kind: u16,
    pub action: u16,
    pub variant: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub enum Animation {
    Inline(AnimationDefinition),
    Predefined(PredefinedAnimation),
}

#[derive(Debug, Clone, Component)]
pub struct HitAnimation {
    pub hit_animation: Animation,
}

#[derive(Debug, Clone, Component)]
pub struct MeleeWeapon {
    pub damage: u16,
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
            .add_system(animation::send_animations.in_set(ServerSet::Send));
    }
}
