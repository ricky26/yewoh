use std::sync::Arc;

use bevy_ecs::prelude::*;
use glam::IVec3;

use yewoh::protocol::{AnyPacket, CreateCharacter, EquipmentSlot, Move, SelectCharacter, UnicodeTextMessageRequest};

#[derive(Debug)]
pub struct ReceivedPacketEvent {
    pub client_entity: Entity,
    pub packet: AnyPacket,
}

#[derive(Debug)]
pub struct SentPacketEvent {
    pub client_entity: Option<Entity>,
    pub packet: Arc<AnyPacket>,
}

#[derive(Debug, Clone)]
pub struct CharacterListEvent {
    pub client_entity: Entity,
}

#[derive(Debug, Clone)]
pub struct CreateCharacterEvent {
    pub client_entity: Entity,
    pub request: CreateCharacter,
}

#[derive(Debug, Clone)]
pub struct SelectCharacterEvent {
    pub client_entity: Entity,
    pub request: SelectCharacter,
}

#[derive(Debug, Clone)]
pub struct MoveEvent {
    pub client_entity: Entity,
    pub request: Move,
}

#[derive(Debug, Clone)]
pub struct SingleClickEvent {
    pub client_entity: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct DoubleClickEvent {
    pub client_entity: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct PickUpEvent {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone)]
pub struct DropEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub position: IVec3,
    pub grid_index: u8,
    pub dropped_on: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct EquipEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub character: Entity,
    pub slot: EquipmentSlot,
}

#[derive(Debug, Clone)]
pub struct NewPrimaryEntityEvent {
    pub client_entity: Entity,
    pub primary_entity: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct ChatRequestEvent {
    pub client_entity: Entity,
    pub request: UnicodeTextMessageRequest,
}

#[derive(Debug, Clone)]
pub struct ContextMenuEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub option: u16,
}

#[derive(Debug, Clone)]
pub struct ProfileEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub new_profile: Option<String>,
}
