use std::io::Write;

use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};
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
    pub title: String,
    pub static_profile: String,
    pub profile: String,
}

#[derive(Debug, Clone)]
pub enum CharacterProfile {
    Request(ProfileRequest),
    Response(ProfileResponse),
}

impl Packet for CharacterProfile {
    fn packet_kind() -> u8 { 0xb8 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        match from_client {
            true => {
                let is_edit = payload.read_u8()? != 0;
                let target_id = payload.read_entity_id()?;
                let new_profile = if is_edit {
                    payload.skip(2)?;
                    Some(payload.read_utf16_nul()?)
                } else {
                    None
                };

                Ok(CharacterProfile::Request(ProfileRequest {
                    target_id,
                    new_profile,
                }))
            }
            false => {
                let target_id = payload.read_entity_id()?;
                let title = payload.read_str_nul()?;
                let static_profile = payload.read_utf16_nul()?;
                let profile = payload.read_utf16_nul()?;
                Ok(CharacterProfile::Response(ProfileResponse {
                    target_id,
                    title,
                    static_profile,
                    profile,
                }))
            }
        }
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        match self {
            CharacterProfile::Request(request) => {
                writer.write_u8(if request.new_profile.is_some() { 1 } else { 0 })?;
                writer.write_entity_id(request.target_id)?;

                if let Some(new_profile) = request.new_profile.as_ref() {
                    writer.write_str_nul(&new_profile)?;
                }
            }
            CharacterProfile::Response(response) => {
                writer.write_entity_id(response.target_id)?;
                writer.write_str_nul(&response.title)?;
                writer.write_utf16_nul(&response.static_profile)?;
                writer.write_utf16_nul(&response.profile)?;
            }
        }

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
    pub skills: Vec<SkillEntry>,
}

#[derive(Debug, Clone)]
pub struct SkillLockRequest {
    pub id: u16,
    pub lock: SkillLock,
}

#[derive(Debug, Clone)]
pub enum Skills {
    Lock(SkillLockRequest),
    Response(SkillsResponse),
}

impl Packet for Skills {
    fn packet_kind() -> u8 { 0x3a }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        if from_client {
            let id = payload.read_u16::<Endian>()?;
            let lock = SkillLock::from_repr(payload.read_u8()?)
                .ok_or_else(|| anyhow!("invalid skill lock"))?;
            Ok(Self::Lock(SkillLockRequest { id, lock }))
        } else {
            let kind = SkillsResponseKind::from_repr(payload.read_u8()?)
                .ok_or_else(|| anyhow!("invalid skills response"))?;
            let mut skills = Vec::new();

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

            Ok(Self::Response(SkillsResponse {
                kind,
                skills,
            }))
        }
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        match self {
            Skills::Lock(request) => {
                writer.write_u16::<Endian>(request.id)?;
                writer.write_u8(request.lock as u8)?;
            }
            Skills::Response(response) => {
                writer.write_u8(response.kind as u8)?;

                for skill in response.skills.iter() {
                    writer.write_u16::<Endian>(skill.id)?;
                    writer.write_u16::<Endian>(skill.value)?;
                    writer.write_u16::<Endian>(skill.raw_value)?;
                    writer.write_u8(skill.lock as u8)?;
                    if response.kind == SkillsResponseKind::FullWithCaps
                        || response.kind == SkillsResponseKind::SingleUpdateWithCap {
                        writer.write_u16::<Endian>(skill.cap)?;
                    }
                }

                if response.kind == SkillsResponseKind::Full {
                    writer.write_u16::<Endian>(0)?;
                }
            }
        }

        Ok(())
    }
}
