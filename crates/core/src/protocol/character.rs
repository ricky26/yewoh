use std::io::Write;

use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};
use smallvec::SmallVec;
use strum_macros::FromRepr;

use crate::EntityId;
use crate::protocol::{PacketReadExt, PacketWriteExt};

use super::{ClientVersion, Endian, Packet};

#[derive(Debug, Clone)]
pub struct ProfileRequest {
    pub target_id: EntityId,
    pub new_profile: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProfileResponse {
    pub target_id: EntityId,
    pub header: String,
    pub footer: String,
    pub profile: String,
}

impl Packet for ProfileRequest {
    const PACKET_KIND: u8 = 0xb8;
    const S2C: bool = false;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let is_edit = payload.read_u8()? != 0;
        let target_id = payload.read_entity_id()?;
        let new_profile = if is_edit {
            payload.skip(2)?;
            Some(payload.read_utf16_pascal()?)
        } else {
            None
        };

        Ok(ProfileRequest {
            target_id,
            new_profile,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(if self.new_profile.is_some() { 1 } else { 0 })?;
        writer.write_entity_id(self.target_id)?;

        if let Some(new_profile) = self.new_profile.as_ref() {
            writer.write_u16::<Endian>(1)?;
            writer.write_utf16_pascal(new_profile)?;
        }
        Ok(())
    }
}

impl Packet for ProfileResponse {
    const PACKET_KIND: u8 = 0xb8;
    const C2S: bool = false;
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_entity_id()?;
        let title = payload.read_str_nul()?;
        let static_profile = payload.read_utf16_nul()?;
        let profile = payload.read_utf16_nul()?;
        Ok(ProfileResponse {
            target_id,
            header: title,
            footer: static_profile,
            profile,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.target_id)?;
        writer.write_str_nul(&self.header)?;
        writer.write_utf16_nul(&self.footer)?;
        writer.write_utf16_nul(&self.profile)?;
        Ok(())
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, FromRepr)]
pub enum SkillsResponseKind {
    Full = 0,
    FullWithCaps = 2,
    SingleUpdate = 0xff,
    SingleUpdateWithCap = 0xdf,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, FromRepr)]
pub enum SkillLock {
    Up = 0,
    Down = 1,
    Locked = 2,
}

#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub id: u16,
    pub value: u16,
    pub raw_value: u16,
    pub lock: SkillLock,
    pub cap: u16,
}

#[derive(Debug, Clone)]
pub struct SkillsResponse {
    pub kind: SkillsResponseKind,
    pub skills: SmallVec<[SkillEntry; 64]>,
}

#[derive(Debug, Clone)]
pub struct SkillLockRequest {
    pub id: u16,
    pub lock: SkillLock,
}

impl Packet for SkillLockRequest {
    const PACKET_KIND: u8 = 0x3a;
    const S2C: bool = false;

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_u16::<Endian>()?;
        let lock = SkillLock::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid skill lock"))?;
        Ok(SkillLockRequest { id, lock })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u16::<Endian>(self.id)?;
        writer.write_u8(self.lock as u8)?;
        Ok(())
    }
}

impl Packet for SkillsResponse {
    const PACKET_KIND: u8 = 0x3a;
    const C2S: bool = false;

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let kind = SkillsResponseKind::from_repr(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid skills response"))?;
        let mut skills = SmallVec::new();

        while payload.len() > 2 {
            let id = payload.read_u16::<Endian>()?;
            let value = payload.read_u16::<Endian>()?;
            let raw_value = payload.read_u16::<Endian>()?;
            let lock = SkillLock::from_repr(payload.read_u8()?)
                .ok_or_else(|| anyhow!("invalid skill lock"))?;
            let cap = if kind == SkillsResponseKind::FullWithCaps || kind == SkillsResponseKind::SingleUpdateWithCap {
                payload.read_u16::<Endian>()?
            } else {
                0
            };
            skills.push(SkillEntry {
                id,
                value,
                raw_value,
                lock,
                cap,
            });
        }

        if kind == SkillsResponseKind::Full {
            payload.skip(2)?;
        }

        Ok(SkillsResponse {
            kind,
            skills,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.kind as u8)?;

        for skill in self.skills.iter() {
            writer.write_u16::<Endian>(skill.id)?;
            writer.write_u16::<Endian>(skill.value)?;
            writer.write_u16::<Endian>(skill.raw_value)?;
            writer.write_u8(skill.lock as u8)?;
            if self.kind == SkillsResponseKind::FullWithCaps
                || self.kind == SkillsResponseKind::SingleUpdateWithCap {
                writer.write_u16::<Endian>(skill.cap)?;
            }
        }

        if self.kind == SkillsResponseKind::Full {
            writer.write_u16::<Endian>(0)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AttackRequest {
    pub target_id: EntityId,
}

impl Packet for AttackRequest {
    const PACKET_KIND: u8 = 0x5;

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(5) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        Ok(Self { target_id: payload.read_entity_id()? })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.target_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SetAttackTarget {
    pub target_id: Option<EntityId>,
}

impl Packet for SetAttackTarget {
    const PACKET_KIND: u8 = 0xaa;

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(5) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let raw_target_id = payload.read_u32::<Endian>()?;
        let target_id = if raw_target_id != 0 {
            Some(EntityId::from_u32(raw_target_id))
        } else {
            None
        };

        Ok(Self { target_id })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        if let Some(id) = self.target_id {
            writer.write_entity_id(id)?;
        } else {
            writer.write_u32::<Endian>(0)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Swing {
    pub attacker_id: EntityId,
    pub target_id: EntityId,
}

impl Packet for Swing {
    const PACKET_KIND: u8 = 0x2f;

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(10) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        payload.read_u8()?;
        let attacker_id = payload.read_entity_id()?;
        let target_id = payload.read_entity_id()?;
        Ok(Self { attacker_id, target_id })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(0)?;
        writer.write_entity_id(self.attacker_id)?;
        writer.write_entity_id(self.target_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CharacterAnimation {
    pub target_id: EntityId,
    pub animation_id: u16,
    pub frame_count: u16,
    pub repeat_count: u16,
    pub reverse: bool,
    pub speed: u8,
}

impl Packet for CharacterAnimation {
    const PACKET_KIND: u8 = 0x6e;

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(14) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_entity_id()?;
        let animation_id = payload.read_u16::<Endian>()?;
        let frame_count = payload.read_u16::<Endian>()?;
        let repeat_count = payload.read_u16::<Endian>()?;
        let reverse = payload.read_u8()? != 0;
        let repeat_count = if payload.read_u8()? != 0 {
            repeat_count
        } else {
            1
        };
        let speed = payload.read_u8()?;

        Ok(Self {
            target_id,
            animation_id,
            frame_count,
            repeat_count,
            reverse,
            speed
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.target_id)?;
        writer.write_u16::<Endian>(self.animation_id)?;
        writer.write_u16::<Endian>(self.frame_count)?;
        writer.write_u16::<Endian>(self.repeat_count)?;
        writer.write_u8(if self.reverse { 1 } else { 0 })?;
        writer.write_u8(if self.repeat_count != 1 { 1 } else { 0 })?;
        writer.write_u8(self.speed)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CharacterPredefinedAnimation {
    pub target_id: EntityId,
    pub kind: u16,
    pub action: u16,
    pub variant: u8,
}

impl Packet for CharacterPredefinedAnimation {
    const PACKET_KIND: u8 = 0xe2;

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(10) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let target_id = payload.read_entity_id()?;
        let kind = payload.read_u16::<Endian>()?;
        let action = payload.read_u16::<Endian>()?;
        let variant = payload.read_u8()?;
        Ok(Self {
            target_id,
            kind,
            action,
            variant,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.target_id)?;
        writer.write_u16::<Endian>(self.kind)?;
        writer.write_u16::<Endian>(self.action)?;
        writer.write_u8(self.variant)?;
        Ok(())
    }
}
