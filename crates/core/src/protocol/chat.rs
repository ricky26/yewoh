use std::io::Write;

use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};
use strum_macros::FromRepr;

use crate::EntityId;
use crate::protocol::{PacketReadExt, PacketWriteExt};
use crate::types::FixedString;

use super::{ClientVersion, Endian, Packet};

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, FromRepr)]
pub enum TextCommandKind {
    #[default]
    Go = 0,
    UseSkill = 0x24,
    CastSpellFromBook = 0x27,
    UseScroll = 0x2f,
    OpenSpellBook = 0x43,
    CastSpellFromMacro = 0x56,
    OpenDoor = 0x58,
    Animate = 0xc7,
    InvokeVirtues = 0xf4,
}

#[derive(Debug, Clone, Default)]
pub struct TextCommand {
    pub kind: TextCommandKind,
    pub command: String,
}

impl Packet for TextCommand {
    const PACKET_KIND: u8 = 0x12;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let kind = TextCommandKind::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid text command kind"))?;
        let command = payload.read_str_nul()?;
        Ok(TextCommand {
            kind,
            command
        })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.kind as u8)?;
        writer.write_str_nul(&self.command)?;
        Ok(())
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, FromRepr)]
pub enum MessageKind
{
    #[default]
    Regular = 0,
    System = 0x1,
    Emote = 0x2,
    Label = 0x6,
    Focus = 0x7,
    Whisper = 0x8,
    Yell = 0x9,
    Spell = 0xa,
    Guild = 0xd,
    Alliance = 0xe,
    Command = 0xf,
}

#[derive(Debug, Clone, Default)]
pub struct AsciiTextMessage {
    pub entity_id: Option<EntityId>,
    pub kind: MessageKind,
    pub text: String,
    pub name: FixedString<30>,
    pub hue: u16,
    pub font: u16,
    pub graphic_id: u16,
}

impl Packet for AsciiTextMessage {
    const PACKET_KIND: u8 = 0x1c;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let raw_entity_id = payload.read_u32::<Endian>()?;
        let entity_id = if raw_entity_id != !0 {
            Some(EntityId::from_u32(raw_entity_id))
        } else {
            None
        };
        let graphic_id = payload.read_u16::<Endian>()?;
        let kind = MessageKind::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid message kind"))?;
        let hue = payload.read_u16::<Endian>()?;
        let font = payload.read_u16::<Endian>()?;
        let name = payload.read_str_fixed()?;
        let text = payload.read_str_nul()?;
        Ok(Self {
            entity_id,
            kind,
            text,
            name,
            hue,
            font,
            graphic_id
        })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        if let Some(entity_id) = self.entity_id {
            writer.write_entity_id(entity_id)?;
        } else {
            writer.write_u32::<Endian>(!0)?;
        }
        writer.write_u16::<Endian>(self.graphic_id)?;
        writer.write_u8(self.kind as u8)?;
        writer.write_u16::<Endian>(self.hue)?;
        writer.write_u16::<Endian>(self.font)?;
        writer.write_str_fixed(&self.name)?;
        writer.write_str_nul(&self.text)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct UnicodeTextMessage {
    pub entity_id: Option<EntityId>,
    pub kind: MessageKind,
    pub language: FixedString<4>,
    pub text: String,
    pub name: FixedString<30>,
    pub hue: u16,
    pub font: u16,
    pub graphic_id: u16,
}

impl Packet for UnicodeTextMessage {
    const PACKET_KIND: u8 = 0xae;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let raw_entity_id = payload.read_u32::<Endian>()?;
        let entity_id = if raw_entity_id != !0 {
            Some(EntityId::from_u32(raw_entity_id))
        } else {
            None
        };
        let graphic_id = payload.read_u16::<Endian>()?;
        let kind = MessageKind::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid message kind"))?;
        let hue = payload.read_u16::<Endian>()?;
        let font = payload.read_u16::<Endian>()?;
        let language = payload.read_str_fixed()?;
        let name = payload.read_str_fixed()?;
        let text = payload.read_utf16_nul()?;
        Ok(Self {
            entity_id,
            kind,
            language,
            text,
            name,
            hue,
            font,
            graphic_id
        })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        if let Some(entity_id) = self.entity_id {
            writer.write_entity_id(entity_id)?;
        } else {
            writer.write_u32::<Endian>(!0)?;
        }
        writer.write_u16::<Endian>(self.graphic_id)?;
        writer.write_u8(self.kind as u8)?;
        writer.write_u16::<Endian>(self.hue)?;
        writer.write_u16::<Endian>(self.font)?;
        writer.write_str_fixed(&self.language)?;
        writer.write_str_fixed(&self.name)?;
        writer.write_utf16_nul(&self.text)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocalisedTextMessage {
    pub entity_id: Option<EntityId>,
    pub graphic_id: u16,
    pub kind: MessageKind,
    pub hue: u16,
    pub font: u16,
    pub name: FixedString<30>,
    pub text_id: u32,
    pub params: String,
}

impl Packet for LocalisedTextMessage {
    const PACKET_KIND: u8 = 0xc1;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let raw_entity_id = payload.read_u32::<Endian>()?;
        let entity_id = if raw_entity_id != !0 {
            Some(EntityId::from_u32(raw_entity_id))
        } else {
            None
        };
        let graphic_id = payload.read_u16::<Endian>()?;
        let kind = MessageKind::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid message kind"))?;
        let hue = payload.read_u16::<Endian>()?;
        let font = payload.read_u16::<Endian>()?;
        let text_id = payload.read_u32::<Endian>()?;
        let name = payload.read_str_fixed()?;
        let params = payload.read_utf16le_nul()?;
        Ok(Self {
            entity_id,
            graphic_id,
            kind,
            name,
            hue,
            font,
            text_id,
            params
        })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        if let Some(entity_id) = self.entity_id {
            writer.write_entity_id(entity_id)?;
        } else {
            writer.write_u32::<Endian>(!0)?;
        }
        writer.write_u16::<Endian>(self.graphic_id)?;
        writer.write_u8(self.kind as u8)?;
        writer.write_u16::<Endian>(self.hue)?;
        writer.write_u16::<Endian>(self.font)?;
        writer.write_u32::<Endian>(self.text_id)?;
        writer.write_str_fixed(&self.name)?;
        writer.write_utf16le_nul(&self.params)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct AsciiTextMessageRequest {
    pub kind: MessageKind,
    pub hue: u16,
    pub font: u16,
    pub text: String,
}

impl Packet for AsciiTextMessageRequest {
    const PACKET_KIND: u8 = 0x03;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let kind = MessageKind::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("unknown message type"))?;
        let hue = payload.read_u16::<Endian>()?;
        let font = payload.read_u16::<Endian>()?;
        let text = payload.read_str_nul()?;
        Ok(Self { kind, hue, font, text })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.kind as u8)?;
        writer.write_u16::<Endian>(self.hue)?;
        writer.write_u16::<Endian>(self.font)?;
        writer.write_str_nul(&self.text)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct UnicodeTextMessageRequest {
    pub kind: MessageKind,
    pub hue: u16,
    pub font: u16,
    pub language: FixedString<4>,
    pub text: String,
    pub keywords: Vec<u16>,
}

impl UnicodeTextMessageRequest {
    const HAS_KEYWORDS: u8 = 0xc0;
}

impl Packet for UnicodeTextMessageRequest {
    const PACKET_KIND: u8 = 0xad;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let kind_raw = payload.read_u8()?;
        let kind = MessageKind::from_repr(kind_raw & 0x3f)
            .ok_or_else(|| anyhow!("unknown message type"))?;
        let hue = payload.read_u16::<Endian>()?;
        let font = payload.read_u16::<Endian>()?;
        let language = payload.read_str_fixed()?;
        let mut keywords = Vec::new();

        let text = if kind_raw & Self::HAS_KEYWORDS != 0 {
            let word = payload.read_u16::<Endian>()?;
            let count = word >> 4;
            let mut bits = word & 0xf;
            let mut have_bits = true;

            for _ in 0..count {
                let id = if have_bits {
                    have_bits = false;
                    (bits << 8) | (payload.read_u8()? as u16)
                } else {
                    let word = payload.read_u16::<Endian>()?;
                    bits = word & 0xf;
                    have_bits = true;
                    word >> 4
                };
                keywords.push(id);
            }

            payload.read_str_nul()?
        } else {
            payload.read_utf16_nul()?
        };

        Ok(Self { kind, hue, font, language, text, keywords })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        let has_keywords = !self.keywords.is_empty();
        let mut raw_kind = self.kind as u8;
        if has_keywords {
            raw_kind |= Self::HAS_KEYWORDS;
        }

        writer.write_u8(raw_kind)?;
        writer.write_u16::<Endian>(self.hue)?;
        writer.write_u16::<Endian>(self.font)?;
        writer.write_str_fixed(&self.language)?;

        if has_keywords {
            let mut to_write = self.keywords.len() as u16;
            let mut have_bits = true;

            for keyword in self.keywords.iter().copied() {
                if have_bits {
                    writer.write_u16::<Endian>((to_write << 4) | (keyword & 0xf))?;
                    to_write = keyword & 0xff;
                } else {
                    writer.write_u16::<Endian>((to_write << 8) | (keyword & 0xff))?;
                    to_write = keyword & 0xfff;
                };
                have_bits = !have_bits;
            }

            writer.write_u16::<Endian>(to_write << if have_bits { 4 } else { 8 })?;
            writer.write_str_nul(&self.text)?;
        } else {
            writer.write_utf16_nul(&self.text)?;
        }

        Ok(())
    }
}
