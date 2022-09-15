use std::io::Write;

use byteorder::{ReadBytesExt, WriteBytesExt};

use crate::EntityId;
use crate::protocol::{PacketReadExt, PacketWriteExt};

use super::{ClientVersion, Packet};

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
