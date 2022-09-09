use std::io::Write;
use anyhow::anyhow;

use byteorder::{ReadBytesExt, WriteBytesExt};
use glam::IVec3;
use crate::{Direction, EntityId, Notoriety};
use crate::protocol::{PacketReadExt, PacketWriteExt};

use super::{ClientVersion, Endian, Packet};

#[derive(Debug, Clone, Default)]
pub struct Move {
    pub direction: Direction,
    pub sequence: u8,
    pub fast_walk: u32,
}

impl Packet for Move {
    fn packet_kind() -> u8 { 0x02 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(7) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let direction = Direction::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("Invalid direction"))?;
        let sequence = payload.read_u8()?;
        let fast_walk = payload.read_u32::<Endian>()?;
        Ok(Move { direction, sequence, fast_walk })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.direction as u8)?;
        writer.write_u8(self.sequence)?;
        writer.write_u32::<Endian>(self.fast_walk)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MoveConfirm {
    pub sequence: u8,
    pub notoriety: Notoriety,
}

impl Packet for MoveConfirm {
    fn packet_kind() -> u8 { 0x22 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(3) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let sequence = payload.read_u8()?;
        let notoriety = Notoriety::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid notoriety"))?;
        Ok(Self { sequence, notoriety })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.sequence)?;
        writer.write_u8(self.notoriety as u8)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MoveReject {
    pub sequence: u8,
    pub position: IVec3,
    pub direction: Direction,
}

impl Packet for MoveReject {
    fn packet_kind() -> u8 { 0x21 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(8) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let sequence = payload.read_u8()?;
        let x = payload.read_u16::<Endian>()? as i32;
        let y = payload.read_u16::<Endian>()? as i32;
        let direction = payload.read_direction()?;
        let z = payload.read_u8()? as i32;
        Ok(Self {
            sequence,
            position: IVec3::new(x, y, z),
            direction
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.sequence)?;
        writer.write_u16::<Endian>(self.position.x as u16)?;
        writer.write_u16::<Endian>(self.position.y as u16)?;
        writer.write_direction(self.direction)?;
        writer.write_u8(self.position.z as u8)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SingleClick {
    pub target_id: EntityId,
}

impl Packet for SingleClick {
    fn packet_kind() -> u8 { 0x9 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(5) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_entity_id()?;
        Ok(Self { target_id })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.target_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DoubleClick {
    pub target_id: EntityId,
}

impl Packet for DoubleClick {
    fn packet_kind() -> u8 { 0x6 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(5) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_entity_id()?;
        Ok(Self { target_id })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.target_id)?;
        Ok(())
    }
}
