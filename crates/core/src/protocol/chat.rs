use strum_macros::FromRepr;

use crate::EntityId;

#[repr(u8)]
#[derive(Debug, Clone, Copy, FromRepr)]
pub enum MessageKind
{
    REGULAR = 0,
    SYSTEM = 0x1,
    EMOTE = 0x2,
    LABEL = 0x6,
    FOCUS = 0x7,
    WHISPER = 0x8,
    YELL = 0x9,
    SPELL = 0xa,
    GUILD = 0xd,
    ALLIANCE = 0xe,
    COMMAND = 0xf,
    ENCODED = 0xc0,
}

impl MessageKind {

}

pub struct AsciiTextMessage {
    entity_id: Option<EntityId>,
    graphic_id: u16,

}