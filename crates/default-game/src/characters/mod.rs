use std::time::Duration;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;

#[derive(Debug, Default, Clone, Component)]
pub struct Alive;

#[derive(Debug, Clone)]
pub struct DamageDealt {
    pub target: Entity,
    pub source: Entity,
    pub damage: u16,
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
