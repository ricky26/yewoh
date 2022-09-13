use std::sync::Arc;

use bevy_ecs::prelude::*;
use glam::IVec3;

use yewoh::protocol::{AnyPacket, CreateCharacter, EquipmentSlot, Move, SelectCharacter, UnicodeTextMessageRequest};

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
pub struct SelectCharacterEvent {
    pub client: Entity,
    pub request: SelectCharacter,
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
pub struct PickUpEvent {
    pub client: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone)]
pub struct DropEvent {
    pub client: Entity,
    pub target: Entity,
    pub position: IVec3,
    pub grid_index: u8,
    pub dropped_on: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct EquipEvent {
    pub client: Entity,
    pub target: Entity,
    pub character: Entity,
    pub slot: EquipmentSlot,
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
