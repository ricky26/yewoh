use std::io::Write;

use byteorder::{ReadBytesExt, WriteBytesExt};

use super::{ClientVersion, Endian, Packet};

#[derive(Debug, Clone)]
pub struct PlayMusic {
    pub track_id: u16,
}

impl Packet for PlayMusic {
    fn packet_kind() -> u8 { 0x6d }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(3) }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let track_id = payload.read_u16::<Endian>()?;
        Ok(Self { track_id })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u16::<Endian>(self.track_id)?;
        Ok(())
    }
}
