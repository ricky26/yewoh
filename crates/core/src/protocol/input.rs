use std::io::Write;

use byteorder::{ReadBytesExt, WriteBytesExt};

use super::{ClientVersion, Endian, Packet};

#[derive(Debug, Clone)]
pub struct SingleClick {
    pub target_id: u32,
}

impl Packet for SingleClick {
    fn packet_kind() -> u8 { 0x9 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(5) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_u32::<Endian>()?;
        Ok(Self { target_id })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.target_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DoubleClick {
    pub target_id: u32,
}

impl Packet for DoubleClick {
    fn packet_kind() -> u8 { 0x6 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(5) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_u32::<Endian>()?;
        Ok(Self { target_id })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.target_id)?;
        Ok(())
    }
}
