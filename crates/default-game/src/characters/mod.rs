use std::time::Duration;
use bevy_app::{App, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use yewoh_server::world::entity::Location;
use crate::characters::prefabs::CharacterPrefab;
use crate::data::prefab::PrefabAppExt;

pub mod prefabs;

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

#[derive(Debug, Default, Clone, Component)]
pub struct CreateCorpse;

#[derive(Debug, Clone)]
pub struct CorpseSpawned {
    pub character: Entity,
    pub corpse: Entity,
}

#[derive(Debug, Clone, Component)]
pub struct Unarmed {
    pub weapon: MeleeWeapon,
}

#[derive(Debug, Clone, Component)]
pub struct MeleeWeapon {
    pub damage: u16,
    pub delay: Duration,
    pub range: i32,
}

pub struct CharactersPlugin;

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_prefab_bundle::<CharacterPrefab>("character");
    }
}
