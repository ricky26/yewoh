use std::io::Write;

use byteorder::{ReadBytesExt, WriteBytesExt};

use super::{ClientVersion, Packet};

#[derive(Debug, Clone)]
pub struct SetTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Packet for SetTime {
    fn packet_kind() -> u8 { 0x5b }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some (4) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let hour = payload.read_u8()?;
        let minute = payload.read_u8()?;
        let second = payload.read_u8()?;
        Ok(SetTime { hour, minute, second })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.hour)?;
        writer.write_u8(self.minute)?;
        writer.write_u8(self.second)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ChangeSeason {
    pub season: u8,
    pub play_sound: bool,
}

impl Packet for ChangeSeason {
    fn packet_kind() -> u8 { 0xbc }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(3) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let season = payload.read_u8()?;
        let play_sound = payload.read_u8()? != 0;
        Ok(ChangeSeason { season, play_sound })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.season)?;
        writer.write_u8(if self.play_sound { 1 } else { 0 })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ViewRange(pub u8);

impl Packet for ViewRange {
    fn packet_kind() -> u8 { 0xc8 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(2) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        Ok(Self(payload.read_u8()?))
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.0)?;
        Ok(())
    }
}

