use std::io::Write;

use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};
use glam::IVec3;
use strum_macros::FromRepr;

use crate::{Direction, EntityId, Notoriety};
use crate::protocol::{PacketReadExt, PacketWriteExt};
use crate::protocol::client_version::VERSION_GRID_INVENTORY;

use super::{ClientVersion, Endian, Packet};

#[derive(Debug, Clone, Default)]
pub struct Move {
    pub direction: Direction,
    pub run: bool,
    pub sequence: u8,
    pub fast_walk: u32,
}

impl Packet for Move {
    fn packet_kind() -> u8 { 0x02 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(7) }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let direction_and_run = payload.read_u8()?;
        let run = (direction_and_run & 0x80) != 0;
        let direction = Direction::from_repr(direction_and_run & 0x7f)
            .ok_or_else(|| anyhow!("Invalid direction"))?;
        let sequence = payload.read_u8()?;
        let fast_walk = payload.read_u32::<Endian>()?;
        Ok(Move { direction, run, sequence, fast_walk })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        let mut direction_and_run = self.direction as u8;
        if self.run {
            direction_and_run |= 0x80;
        }
        writer.write_u8(direction_and_run)?;
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

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let sequence = payload.read_u8()?;
        let notoriety = Notoriety::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid notoriety"))?;
        Ok(Self { sequence, notoriety })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
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

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let sequence = payload.read_u8()?;
        let x = payload.read_u16::<Endian>()? as i32;
        let y = payload.read_u16::<Endian>()? as i32;
        let direction = payload.read_direction()?;
        let z = payload.read_u8()? as i32;
        Ok(Self {
            sequence,
            position: IVec3::new(x, y, z),
            direction,
        })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
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

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_entity_id()?;
        Ok(Self { target_id })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.target_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DoubleClick {
    pub target_id: EntityId,
    pub paperdoll: bool,
}

impl Packet for DoubleClick {
    fn packet_kind() -> u8 { 0x6 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(5) }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let raw_target_id = payload.read_u32::<Endian>()?;
        let target_id = EntityId::from_u32(raw_target_id & 0x7fffffff);
        let paperdoll = (raw_target_id & 0x80000000) != 0;
        Ok(Self { target_id, paperdoll })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        let mut target_id = self.target_id.as_u32();
        if self.paperdoll {
            target_id |= 0x80000000;
        }
        writer.write_u32::<Endian>(target_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PickUpEntity {
    pub target_id: EntityId,
    pub quantity: u16,
}

impl Packet for PickUpEntity {
    fn packet_kind() -> u8 { 0x7 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(7) }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_entity_id()?;
        let quantity = payload.read_u16::<Endian>()?;
        Ok(Self { target_id, quantity })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.target_id)?;
        writer.write_u16::<Endian>(self.quantity)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DropEntity {
    pub target_id: EntityId,
    pub position: IVec3,
    pub grid_index: u8,
    pub container_id: Option<EntityId>,
}

impl Packet for DropEntity {
    fn packet_kind() -> u8 { 0x8 }

    fn fixed_length(client_version: ClientVersion) -> Option<usize> {
        Some(if client_version >= VERSION_GRID_INVENTORY { 15 } else { 14 })
    }

    fn decode(client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_entity_id()?;
        let x = payload.read_u16::<Endian>()? as i32;
        let y = payload.read_u16::<Endian>()? as i32;
        let z = payload.read_u8()? as i32;
        let grid_index = if client_version >= VERSION_GRID_INVENTORY {
            payload.read_u8()?
        } else {
            0
        };
        let raw_container_id = payload.read_u32::<Endian>()?;
        let container_id = if raw_container_id == !0 {
            None
        } else {
            Some(EntityId::from_u32(raw_container_id))
        };
        Ok(Self {
            target_id,
            position: IVec3::new(x, y, z),
            grid_index,
            container_id,
        })
    }

    fn encode(&self, client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.target_id)?;
        writer.write_u16::<Endian>(self.position.x as u16)?;
        writer.write_u16::<Endian>(self.position.x as u16)?;
        writer.write_u8(self.position.z as u8)?;
        if client_version >= VERSION_GRID_INVENTORY {
            writer.write_u8(self.grid_index)?;
        }
        if let Some(container_id) = self.container_id {
            writer.write_entity_id(container_id)?;
        } else {
            writer.write_u32::<Endian>(!0)?;
        }
        Ok(())
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, FromRepr)]
pub enum MoveEntityReject {
    CannotLift = 0,
    OutOfRange = 1,
    OutOfSight = 2,
    BelongsToAnother = 3,
    AlreadyHolding = 4,
}

impl Packet for MoveEntityReject {
    fn packet_kind() -> u8 { 0x27 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(2) }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        Ok(MoveEntityReject::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("Invalid rejection reason"))?)
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(*self as u8)?;
        Ok(())
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, FromRepr)]
pub enum TargetType {
    Neutral = 0,
    Harmful = 1,
    Helpful = 2,
    Cancel = 3,
}

#[derive(Debug, Clone)]
pub struct PickTarget {
    pub target_ground: bool,
    pub target_type: TargetType,
    pub id: u32,
    pub target_id: Option<EntityId>,
    pub position: IVec3,
    pub graphic_id: u16,
}

impl Packet for PickTarget {
    fn packet_kind() -> u8 { 0x6c }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(19) }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_ground = payload.read_u8()? != 0;
        let id = payload.read_u32::<Endian>()?;
        let target_type = TargetType::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid target type"))?;
        let target_id = Some(payload.read_entity_id()?);
        let x = payload.read_u16::<Endian>()? as i32;
        let y = payload.read_u16::<Endian>()? as i32;
        let z = payload.read_u16::<Endian>()? as i32;
        let graphic_id = payload.read_u16::<Endian>()?;
        Ok(Self {
            target_ground,
            target_type,
            id,
            target_id,
            position: IVec3::new(x, y, z),
            graphic_id,
        })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(if self.target_ground { 1 } else { 0 })?;
        writer.write_u32::<Endian>(self.id)?;
        writer.write_u8(self.target_type as u8)?;
        if let Some(target_id) = self.target_id {
            writer.write_entity_id(target_id)?;
        } else {
            writer.write_u32::<Endian>(0)?;
        }
        writer.write_u16::<Endian>(self.position.x as u16)?;
        writer.write_u16::<Endian>(self.position.y as u16)?;
        writer.write_u16::<Endian>(self.position.z as u16)?;
        writer.write_u16::<Endian>(self.graphic_id)?;
        Ok(())
    }
}
