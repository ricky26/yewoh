use bevy_ecs::prelude::*;
use yewoh::protocol::{AnyPacket, CreateCharacter, Move, UnicodeTextMessageRequest};

#[derive(Debug)]
pub struct ReceivedPacketEvent {
    pub connection: Entity,
    pub packet: AnyPacket,
}

#[derive(Debug)]
pub struct SentPacketEvent {
    pub connection: Entity,
    pub packet: AnyPacket,
}

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
    pub request: Move,
}

#[derive(Debug, Clone)]
pub struct SingleClickEvent {
    pub connection: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct DoubleClickEvent {
    pub connection: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct NewPrimaryEntityEvent {
    pub connection: Entity,
    pub primary_entity: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct ChatRequestEvent {
    pub connection: Entity,
    pub request: UnicodeTextMessageRequest,
}
