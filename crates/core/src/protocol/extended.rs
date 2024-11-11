use std::io::Write;

use anyhow::anyhow;
use bitflags::bitflags;
use byteorder::{ReadBytesExt, WriteBytesExt};
use smallvec::SmallVec;
use tracing::warn;

use crate::protocol::{ClientFlags, PacketReadExt, PacketWriteExt};
use crate::EntityId;

use super::{ClientVersion, Endian, Packet};

#[derive(Debug, Clone)]
pub struct CloseGump {
    pub gump_id: u32,
    pub button_id: u32,
}

#[derive(Debug, Clone)]
pub struct ScreenSize {
    pub width: u32,
    pub height: u32,
}

bitflags! {
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ContextMenuFlags : u16 {
        const DISABLED = 0x01;
        const ARROW = 0x02;
        const HIGHLIGHTED = 0x04;
        const HUE = 0x20;
    }
}

#[derive(Debug, Clone)]
pub struct ContextMenuEntry {
    pub id: u16,
    pub text_id: u32,
    pub hue: Option<u16>,
    pub flags: ContextMenuFlags,
}

#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub target_id: EntityId,
    pub entries: SmallVec<[ContextMenuEntry; 16]>,
}

#[derive(Debug, Clone)]
pub struct ContextMenuResponse {
    pub target_id: EntityId,
    pub id: u16,
}

#[derive(Debug, Clone)]
pub enum ExtendedCommand {
    Unknown(u16),
    CloseGump(CloseGump),
    ScreenSize(ScreenSize),
    ChangeMap(u8),
    Language(String),
    CloseStatusGump(u32),
    ClientType(ClientFlags),
    ContextMenuRequest(EntityId),
    ContextMenu(ContextMenu),
    ContextMenuEnhanced(ContextMenu),
    ContextMenuResponse(ContextMenuResponse),
}

impl ExtendedCommand {
    const CLOSE_GUMP: u16 = 0x4;
    const SCREEN_SIZE: u16 = 0x5;
    const CHANGE_MAP: u16 = 0x8;
    const LANGUAGE: u16 = 0xb;
    const CLOSE_STATUS_GUMP: u16 = 0xc;
    const CLIENT_TYPE: u16 = 0xf;
    const CONTEXT_MENU_REQUEST: u16 = 0x13;
    const CONTEXT_MENU: u16 = 0x14;
    const CONTEXT_MENU_RESPONSE: u16 = 0x15;

    const CLASSIC_CONTEXT_MIN_TEXT_ID: u32 = 3000000;

    pub fn kind(&self) -> u16 {
        match self {
            ExtendedCommand::Unknown(_) => panic!("Tried to send unknown extended command"),
            ExtendedCommand::CloseGump(_) => Self::CLOSE_GUMP,
            ExtendedCommand::ScreenSize(_) => Self::SCREEN_SIZE,
            ExtendedCommand::ChangeMap(_) => Self::CHANGE_MAP,
            ExtendedCommand::Language(_) => Self::LANGUAGE,
            ExtendedCommand::CloseStatusGump(_) => Self::CLOSE_STATUS_GUMP,
            ExtendedCommand::ClientType(_) => Self::CLIENT_TYPE,
            ExtendedCommand::ContextMenuRequest(_) => Self::CONTEXT_MENU_REQUEST,
            ExtendedCommand::ContextMenu(_) => Self::CONTEXT_MENU,
            ExtendedCommand::ContextMenuEnhanced(_) => Self::CONTEXT_MENU,
            ExtendedCommand::ContextMenuResponse(_) => Self::CONTEXT_MENU_RESPONSE,
        }
    }
}

impl Packet for ExtendedCommand {
    const PACKET_KIND: u8 = 0xbf;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let kind = payload.read_u16::<Endian>()?;
        match kind {
            Self::CLOSE_GUMP => Ok(ExtendedCommand::CloseGump(CloseGump {
                gump_id: payload.read_u32::<Endian>()?,
                button_id: payload.read_u32::<Endian>()?,
            })),
            Self::SCREEN_SIZE => Ok(ExtendedCommand::ScreenSize(ScreenSize {
                width: payload.read_u32::<Endian>()?,
                height: payload.read_u32::<Endian>()?,
            })),
            Self::CHANGE_MAP => Ok(ExtendedCommand::ChangeMap(payload.read_u8()?)),
            Self::LANGUAGE => Ok(ExtendedCommand::Language(payload.read_str_nul()?)),
            Self::CLOSE_STATUS_GUMP =>
                Ok(ExtendedCommand::CloseStatusGump(payload.read_u32::<Endian>()?)),
            Self::CLIENT_TYPE => Ok(ExtendedCommand::ClientType(
                ClientFlags::from_bits_truncate(payload.read_u32::<Endian>()?))),
            Self::CONTEXT_MENU_REQUEST => Ok(ExtendedCommand::ContextMenuRequest(payload.read_entity_id()?)),
            Self::CONTEXT_MENU => {
                let subcommand = payload.read_u16::<Endian>()?;
                let target_id = payload.read_entity_id()?;
                let count = payload.read_u8()? as usize;
                let mut entries = SmallVec::new();

                match subcommand {
                    1 => {
                        for _ in 0..count {
                            let id = payload.read_u16::<Endian>()?;
                            let text_id = payload.read_u16::<Endian>()? as u32
                                + Self::CLASSIC_CONTEXT_MIN_TEXT_ID;
                            let flags = ContextMenuFlags::from_bits_truncate(payload.read_u16::<Endian>()?);
                            let hue = if flags.contains(ContextMenuFlags::HUE) {
                                Some(payload.read_u16::<Endian>()?)
                            } else {
                                None
                            };
                            entries.push(ContextMenuEntry { id, text_id, flags, hue });
                        }

                        Ok(ExtendedCommand::ContextMenu(ContextMenu { target_id, entries }))
                    }
                    2 => {
                        for _ in 0..count {
                            let text_id = payload.read_u32::<Endian>()?;
                            let id = payload.read_u16::<Endian>()?;
                            let flags = ContextMenuFlags::from_bits_truncate(payload.read_u16::<Endian>()?);
                            entries.push(ContextMenuEntry { id, text_id, flags, hue: None });
                        }

                        Ok(ExtendedCommand::ContextMenuEnhanced(ContextMenu { target_id, entries }))
                    }
                    _ => Ok(ExtendedCommand::Unknown(Self::CONTEXT_MENU)),
                }
            }
            Self::CONTEXT_MENU_RESPONSE => {
                let target_id = payload.read_entity_id()?;
                let id = payload.read_u16::<Endian>()?;
                Ok(ExtendedCommand::ContextMenuResponse(ContextMenuResponse { id, target_id }))
            }
            c => {
                warn!("Unknown extended packet {kind}");
                Ok(ExtendedCommand::Unknown(c))
            }
        }
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u16::<Endian>(self.kind())?;
        match self {
            ExtendedCommand::Unknown(_) =>
                return Err(anyhow!("tried to send unknown extended command")),
            ExtendedCommand::CloseGump(close_gump) => {
                writer.write_u32::<Endian>(close_gump.gump_id)?;
                writer.write_u32::<Endian>(close_gump.button_id)?;
            }
            ExtendedCommand::ScreenSize(screen_size) => {
                writer.write_u32::<Endian>(screen_size.width)?;
                writer.write_u32::<Endian>(screen_size.height)?;
            }
            ExtendedCommand::ChangeMap(map) =>
                writer.write_u8(*map)?,
            ExtendedCommand::Language(language) =>
                writer.write_str_nul(language)?,
            ExtendedCommand::CloseStatusGump(id) =>
                writer.write_u32::<Endian>(*id)?,
            ExtendedCommand::ClientType(client_type) =>
                writer.write_u32::<Endian>(client_type.bits())?,
            ExtendedCommand::ContextMenuRequest(target_id) =>
                writer.write_entity_id(*target_id)?,
            ExtendedCommand::ContextMenu(menu) => {
                writer.write_u16::<Endian>(1)?;
                writer.write_entity_id(menu.target_id)?;
                writer.write_u8(menu.entries.len() as u8)?;

                for entry in menu.entries.iter() {
                    if entry.text_id < Self::CLASSIC_CONTEXT_MIN_TEXT_ID {
                        return Err(anyhow!("Class context menu must only contain text IDs > {}",
                            Self::CLASSIC_CONTEXT_MIN_TEXT_ID));
                    }

                    let mut flags = entry.flags & !ContextMenuFlags::HUE;
                    if entry.hue.is_some() {
                        flags |= ContextMenuFlags::HUE;
                    }

                    writer.write_u16::<Endian>(entry.id)?;
                    writer.write_u16::<Endian>((entry.text_id - Self::CLASSIC_CONTEXT_MIN_TEXT_ID) as u16)?;
                    writer.write_u16::<Endian>(entry.flags.bits())?;

                    if let Some(hue) = entry.hue {
                        writer.write_u16::<Endian>(hue)?;
                    }
                }
            }
            ExtendedCommand::ContextMenuEnhanced(menu) => {
                writer.write_u16::<Endian>(2)?;
                writer.write_entity_id(menu.target_id)?;
                writer.write_u8(menu.entries.len() as u8)?;

                for entry in menu.entries.iter() {
                    writer.write_u32::<Endian>(entry.text_id)?;
                    writer.write_u16::<Endian>(entry.id)?;
                    writer.write_u16::<Endian>(entry.flags.bits())?;
                }
            }
            ExtendedCommand::ContextMenuResponse(response) => {
                writer.write_entity_id(response.target_id)?;
                writer.write_u16::<Endian>(response.id)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ExtendedCommandAos {
    Unknown,
}

impl Packet for ExtendedCommandAos {
    const PACKET_KIND: u8 = 0xd7;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, _payload: &[u8]) -> anyhow::Result<Self> {
        Ok(Self::Unknown)
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, _writer: &mut impl Write) -> anyhow::Result<()> {
        Ok(())
    }
}
