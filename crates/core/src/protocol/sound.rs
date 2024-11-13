use std::io::Write;

use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};
use glam::IVec3;
use strum_macros::FromRepr;

use crate::protocol::PacketReadExt;

use super::{ClientVersion, Endian, Packet};

#[derive(Debug, Clone)]
pub struct PlayMusic {
    pub track_id: u16,
}

impl Packet for PlayMusic {
    const PACKET_KIND: u8 = 0x6d;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(3) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let track_id = payload.read_u16::<Endian>()?;
        Ok(Self { track_id })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u16::<Endian>(self.track_id)?;
        Ok(())
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, FromRepr)]
pub enum SoundEffectKind {
    Ambiance = 0,
    #[default]
    OneShot = 1,
}

#[derive(Debug, Clone)]
pub struct PlaySoundEffect {
    pub kind: SoundEffectKind,
    pub sound_effect_id: u16,
    pub position: IVec3,
}

impl Packet for PlaySoundEffect {
    const PACKET_KIND: u8 = 0x54;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(12) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let kind = SoundEffectKind::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid sound effect kind"))?;
        let sound_effect_id = payload.read_u16::<Endian>()?;
        payload.skip(2)?;
        let x = payload.read_u16::<Endian>()? as i32;
        let y = payload.read_u16::<Endian>()? as i32;
        let z = payload.read_u16::<Endian>()? as i32;
        Ok(Self {
            kind,
            sound_effect_id,
            position: IVec3::new(x, y, z),
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.kind as u8)?;
        writer.write_u16::<Endian>(self.sound_effect_id)?;
        writer.write_u16::<Endian>(0)?;
        writer.write_u16::<Endian>(self.position.x as u16)?;
        writer.write_u16::<Endian>(self.position.y as u16)?;
        writer.write_u16::<Endian>(self.position.z as u16)?;
        Ok(())
    }
}
