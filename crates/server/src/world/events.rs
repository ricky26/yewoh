use bevy_ecs::prelude::*;
use yewoh::protocol::{CreateCharacter, Move, UnicodeTextMessageRequest};

#[derive(Debug, Clone)]
pub struct CharacterListEvent {
    pub connection: Entity,
}

#[derive(Debug, Clone)]
pub struct CreateCharacterEvent {
    pub connection: Entity,
    pub request: CreateCharacter,
}

#[derive(Debug, Clone)]
pub struct MoveEvent {
    pub connection: Entity,
    pub primary_entity: Option<Entity>,
    pub request: Move,
}

#[derive(Debug, Clone)]
pub struct NewPrimaryEntityEvent {
    pub connection: Entity,
    pub primary_entity: Entity,
}

#[derive(Debug, Clone)]
pub struct ChatRequestEvent {
    pub connection: Entity,
    pub request: UnicodeTextMessageRequest,
}
