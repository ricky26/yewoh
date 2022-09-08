use std::io::Write;

use byteorder::{ReadBytesExt, WriteBytesExt};

use crate::protocol::PacketReadExt;

use super::{ClientVersion, Endian, Packet};

pub const REQUEST_MOBILE_STATUS: u8 = 4;
pub const REQUEST_MOBILE_SKILLS: u8 = 4;

#[derive(Debug, Clone)]
pub struct MobileRequest {
    kind: u8,
    target: u32,
}

impl Packet for MobileRequest {
    fn packet_kind() -> u8 { 0x34 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(10) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        payload.skip(4)?;
        let kind = payload.read_u8()?;
        let target = payload.read_u32::<Endian>()?;
        Ok(MobileRequest { kind, target })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(0xedededed)?;
        writer.write_u8(self.kind)?;
        writer.write_u32::<Endian>(self.target)?;
        Ok(())
    }
}
