use std::sync::Arc;
use bevy_ecs::prelude::*;
use yewoh::protocol::{AnyPacket, CreateCharacter, Move, UnicodeTextMessageRequest};

#[derive(Debug)]
pub struct ReceivedPacketEvent {
    pub client: Entity,
    pub packet: AnyPacket,
}

#[derive(Debug)]
pub struct SentPacketEvent {
    pub client: Option<Entity>,
    pub packet: Arc<AnyPacket>,
}

#[derive(Debug, Clone)]
pub struct CharacterListEvent {
    pub client: Entity,
}

#[derive(Debug, Clone)]
pub struct CreateCharacterEvent {
    pub client: Entity,
    pub request: CreateCharacter,
}

#[derive(Debug, Clone)]
pub struct MoveEvent {
    pub client: Entity,
    pub request: Move,
}

#[derive(Debug, Clone)]
pub struct SingleClickEvent {
    pub client: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct DoubleClickEvent {
    pub client: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct NewPrimaryEntityEvent {
    pub client: Entity,
    pub primary_entity: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct ChatRequestEvent {
    pub client: Entity,
    pub request: UnicodeTextMessageRequest,
}
