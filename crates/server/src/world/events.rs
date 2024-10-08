use std::sync::Arc;

use bevy::prelude::*;

use yewoh::protocol::{AnyPacket, CreateCharacter, DeleteCharacter, EquipmentSlot, Move, SelectCharacter, UnicodeTextMessageRequest};

#[derive(Debug, Event)]
pub struct ReceivedPacketEvent {
    pub client_entity: Entity,
    pub packet: AnyPacket,
}

#[derive(Debug, Event)]
pub struct SentPacketEvent {
    pub client_entity: Option<Entity>,
    pub packet: Arc<AnyPacket>,
}

#[derive(Debug, Clone, Event)]
pub struct CharacterListEvent {
    pub client_entity: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct CreateCharacterEvent {
    pub client_entity: Entity,
    pub request: CreateCharacter,
}

#[derive(Debug, Clone, Event)]
pub struct SelectCharacterEvent {
    pub client_entity: Entity,
    pub request: SelectCharacter,
}

#[derive(Debug, Clone, Event)]
pub struct DeleteCharacterEvent {
    pub client_entity: Entity,
    pub request: DeleteCharacter,
}

#[derive(Debug, Clone, Event)]
pub struct MoveEvent {
    pub client_entity: Entity,
    pub request: Move,
}

#[derive(Debug, Clone, Event)]
pub struct SingleClickEvent {
    pub client_entity: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct DoubleClickEvent {
    pub client_entity: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct PickUpEvent {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct DropEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub position: IVec3,
    pub grid_index: u8,
    pub dropped_on: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct EquipEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub character: Entity,
    pub slot: EquipmentSlot,
}

#[derive(Debug, Clone, Event)]
pub struct ChatRequestEvent {
    pub client_entity: Entity,
    pub request: UnicodeTextMessageRequest,
}

#[derive(Debug, Clone, Event)]
pub struct ContextMenuEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub option: u16,
}

#[derive(Debug, Clone, Event)]
pub struct ProfileEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub new_profile: Option<String>,
}

#[derive(Debug, Clone, Event)]
pub struct RequestSkillsEvent {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct AttackRequestedEvent {
    pub client_entity: Entity,
    pub target: Entity,
}
