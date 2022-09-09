use std::io::Write;

use byteorder::{ReadBytesExt, WriteBytesExt};

use crate::EntityId;
use crate::protocol::{EntityFlags, PacketReadExt, PacketWriteExt};

use super::{ClientVersion, Packet, Endian};

#[derive(Debug, Clone)]
pub struct OpenChatWindow;

impl Packet for OpenChatWindow {
    fn packet_kind() -> u8 { 0xb5 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(64) }

    fn decode(_client_version: ClientVersion, _from_client: bool, _payload: &[u8]) -> anyhow::Result<Self> {
        Ok(Self)
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_zeros(63)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct OpenPaperDoll {
    pub id: EntityId,
    pub text: String,
    pub flags: EntityFlags,
}

impl Packet for OpenPaperDoll {
    fn packet_kind() -> u8 { 0x88 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(66) }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_entity_id()?;
        let text = payload.read_str_block(60)?;
        let flags = EntityFlags::from_bits_truncate(payload.read_u8()?);
        Ok(Self { id, text, flags })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.id)?;
        writer.write_str_block(&self.text, 60)?;
        writer.write_u8(self.flags.bits())?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct OpenContainer {
    pub id: EntityId,
    pub gump_id: u16,
}

impl OpenContainer {
    const MIN_VERSION_EXTRA_FIELD: ClientVersion = ClientVersion::new(7, 0, 9, 0);
}

impl Packet for OpenContainer {
    fn packet_kind() -> u8 { 0x24 }

    fn fixed_length(client_version: ClientVersion) -> Option<usize> {
        Some(if client_version > Self::MIN_VERSION_EXTRA_FIELD { 9 } else { 7 })
    }

    fn decode(client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_entity_id()?;
        let gump_id = payload.read_u16::<Endian>()?;
        if client_version > Self::MIN_VERSION_EXTRA_FIELD {
            payload.skip(2)?;
        }
        Ok(Self { id, gump_id })
    }

    fn encode(&self, client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.id)?;
        writer.write_u16::<Endian>(self.gump_id)?;
        if client_version > Self::MIN_VERSION_EXTRA_FIELD {
            writer.write_u16::<Endian>(0x7d)?;
        }
        Ok(())
    }
}
