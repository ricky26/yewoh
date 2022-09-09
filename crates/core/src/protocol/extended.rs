use std::io::Write;

use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};

use crate::protocol::{ClientFlags, PacketReadExt, PacketWriteExt};

use super::{ClientVersion, Endian, Packet};

#[derive(Debug, Clone)]
pub struct ScreenSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub enum ExtendedCommand {
    Unknown,
    ScreenSize(ScreenSize),
    ChangeMap(u8),
    Language(String),
    CloseStatusGump(u32),
    ClientType(ClientFlags),
}

impl ExtendedCommand {
    const SCREEN_SIZE: u16 = 0x5;
    const CHANGE_MAP: u16 = 0x8;
    const LANGUAGE: u16 = 0xb;
    const CLOSE_STATUS_GUMP: u16 = 0xc;
    const CLIENT_TYPE: u16 = 0xf;

    pub fn kind(&self) -> u16 {
        match self {
            ExtendedCommand::Unknown => panic!("Tried to send unknown extended command"),
            ExtendedCommand::ScreenSize(_) => Self::SCREEN_SIZE,
            ExtendedCommand::ChangeMap(_) => Self::CHANGE_MAP,
            ExtendedCommand::Language(_) => Self::LANGUAGE,
            ExtendedCommand::CloseStatusGump(_) => Self::CLOSE_STATUS_GUMP,
            ExtendedCommand::ClientType(_) => Self::CLIENT_TYPE,
        }
    }
}

impl Packet for ExtendedCommand {
    fn packet_kind() -> u8 { 0xbf }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let kind = payload.read_u16::<Endian>()?;
        match kind {
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
            _ => {
                log::warn!("Unknown extended packet {kind}");
                Ok(ExtendedCommand::Unknown)
            },
        }
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u16::<Endian>(self.kind())?;
        match self {
            ExtendedCommand::Unknown => return Err(anyhow!("tried to send unknown extended command")),
            ExtendedCommand::ScreenSize(screen_size) => {
                writer.write_u32::<Endian>(screen_size.width)?;
                writer.write_u32::<Endian>(screen_size.height)?;
            }
            ExtendedCommand::ChangeMap(map) =>
                writer.write_u8(*map)?,
            ExtendedCommand::Language(language) =>
                writer.write_str_nul(&language)?,
            ExtendedCommand::CloseStatusGump(id) =>
                writer.write_u32::<Endian>(*id)?,
            ExtendedCommand::ClientType(client_type) =>
                writer.write_u32::<Endian>(client_type.bits())?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ExtendedCommandAos {
    Unknown,
}

impl Packet for ExtendedCommandAos {
    fn packet_kind() -> u8 { 0xd7 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, _payload: &[u8]) -> anyhow::Result<Self> {
        Ok(Self::Unknown)
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, _writer: &mut impl Write) -> anyhow::Result<()> {
        Ok(())
    }
}
