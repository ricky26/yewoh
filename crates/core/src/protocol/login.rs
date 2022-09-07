use std::io::Write;

use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};

use crate::protocol::{PacketReadExt, PacketWriteExt};

use super::{ClientFlags, ClientVersion, Endian, Packet};

#[derive(Debug, Clone, Default)]
pub struct RegionLogin {
    pub username: String,
    pub password: String,
}

impl Packet for RegionLogin {
    fn packet_kind() -> u8 { 0x80 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(0x3d) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let username = payload.read_str_block(30)?;
        let password = payload.read_str_block(30)?;
        payload.skip(1)?;

        Ok(RegionLogin {
            username,
            password,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_str_block(&self.username, 30)?;
        writer.write_str_block(&self.password, 30)?;
        writer.write_u8(0xff)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SelectGameServer {
    server_id: u8,
}

impl Packet for SelectGameServer {
    fn packet_kind() -> u8 { 0xA0 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(2) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        payload.skip(1)?;
        let server_id = payload.read_u8()?;
        Ok(Self {
            server_id,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(0)?;
        writer.write_u8(self.server_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GameServerLogin {
    pub seed: u32,
    pub username: String,
    pub password: String,
}

impl Packet for GameServerLogin {
    fn packet_kind() -> u8 { 0x91 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(0x40) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let seed = payload.read_u32::<Endian>()?;
        let username = payload.read_str_block(30)?;
        let password = payload.read_str_block(30)?;
        Ok(GameServerLogin {
            seed,
            username,
            password,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.seed)?;
        writer.write_str_block(&self.username, 30)?;
        writer.write_str_block(&self.password, 30)?;
        Ok(())
    }
}

const CREATE_CHARACTER_MAGIC_1: u32 = 0xedededed;
const CREATE_CHARACTER_MAGIC_2: u32 = 0xffffffff;

#[derive(Debug, Clone, Copy, Default)]
pub struct InitialSkill {
    pub skill_id: u8,
    pub points: u8,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InitialCharacterVisual {
    pub graphic: u16,
    pub hue: u16,
}

#[derive(Debug, Clone)]
pub struct CreateCharacter {
    client_flags: ClientFlags,
    character_name: String,
    profession: u8,
    is_female: bool,
    race: u8,
    str: u8,
    dex: u8,
    int: u8,
    skills: [InitialSkill; 4],
    hue: u16,
    hair: InitialCharacterVisual,
    beard: InitialCharacterVisual,
    shirt_hue: u16,
    pants_hue: u16,
    city_index: u16,
    slot: u16,
    ip: u32,
}

impl CreateCharacter {
    const CLIENT_MIN_VERSION_RACE: ClientVersion = ClientVersion::new(4, 0, 11, 4);

    fn decode(extended: bool, client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let num_skills = if extended { 4 } else { 3 };

        if payload.read_u32::<Endian>()? != CREATE_CHARACTER_MAGIC_1 {
            return Err(anyhow!("Bad create character magic 1"));
        }

        if payload.read_u32::<Endian>()? != CREATE_CHARACTER_MAGIC_2 {
            return Err(anyhow!("Bad create character magic 2"));
        }

        payload.skip(1)?;
        let character_name = payload.read_str_block(30)?;
        payload.skip(2)?;

        let client_flags = ClientFlags::from_bits_truncate(payload.read_u32::<Endian>()?);
        payload.skip(8)?;
        let profession = payload.read_u8()?;
        payload.skip(15)?;

        let race_and_gender = payload.read_u8()?;
        let is_female = race_and_gender & 1 != 0;
        let race = if client_version.major >= 7 {
            race_and_gender >> 1
        } else if client_version > Self::CLIENT_MIN_VERSION_RACE {
            (race_and_gender >> 1) - 1
        } else {
            0
        };

        let str = payload.read_u8()?;
        let dex = payload.read_u8()?;
        let int = payload.read_u8()?;

        let mut skills = [InitialSkill::default(); 4];
        for skill in skills.iter_mut().take(num_skills) {
            skill.skill_id = payload.read_u8()?;
            skill.points = payload.read_u8()?;
        }

        let hue = payload.read_u16::<Endian>()?;
        let hair = InitialCharacterVisual {
            graphic: payload.read_u16::<Endian>()?,
            hue: payload.read_u16::<Endian>()?,
        };
        let beard = InitialCharacterVisual {
            graphic: payload.read_u16::<Endian>()?,
            hue: payload.read_u16::<Endian>()?,
        };
        let city_index = payload.read_u16::<Endian>()?;
        payload.skip(2)?;
        let slot = payload.read_u16::<Endian>()?;
        let ip = payload.read_u32::<Endian>()?;
        let shirt_hue = payload.read_u16::<Endian>()?;
        let pants_hue = payload.read_u16::<Endian>()?;

        Ok(CreateCharacter {
            client_flags,
            character_name,
            profession,
            is_female,
            race,
            str,
            dex,
            int,
            skills,
            hue,
            hair,
            beard,
            city_index,
            slot,
            ip,
            shirt_hue,
            pants_hue,
        })
    }

    fn encode(&self, extended: bool, client_version: ClientVersion, writer: &mut impl Write)
        -> anyhow::Result<()> {
        let num_skills = if extended { 4 } else { 3 };

        writer.write_u32::<Endian>(CREATE_CHARACTER_MAGIC_1)?;
        writer.write_u32::<Endian>(CREATE_CHARACTER_MAGIC_2)?;
        writer.write_zeros(1)?;
        writer.write_str_block(&self.character_name, 30)?;
        writer.write_zeros(2)?;
        writer.write_u32::<Endian>(self.client_flags.bits())?;
        writer.write_u32::<Endian>(1)?;
        writer.write_u32::<Endian>(0)?;
        writer.write_u8(self.profession)?;
        writer.write_zeros(15)?;

        let mut race_and_gender = if self.is_female { 1 } else { 0 };
        if client_version.major >= 7 {
            race_and_gender |= self.race << 1;
        } else if client_version >= Self::CLIENT_MIN_VERSION_RACE {
            race_and_gender |= (self.race << 1) + 1;
        }
        writer.write_u8(race_and_gender)?;
        writer.write_u8(self.str)?;
        writer.write_u8(self.dex)?;
        writer.write_u8(self.int)?;

        for skill in self.skills.iter().take(num_skills) {
            writer.write_u8(skill.skill_id)?;
            writer.write_u8(skill.points)?;
        }

        writer.write_u16::<Endian>(self.hue)?;
        writer.write_u16::<Endian>(self.hair.graphic)?;
        writer.write_u16::<Endian>(self.hair.hue)?;
        writer.write_u16::<Endian>(self.beard.graphic)?;
        writer.write_u16::<Endian>(self.beard.hue)?;
        writer.write_u16::<Endian>(self.city_index)?;
        writer.write_u16::<Endian>(0)?;
        writer.write_u16::<Endian>(self.slot)?;
        writer.write_u32::<Endian>(self.ip)?;
        writer.write_u16::<Endian>(self.shirt_hue)?;
        writer.write_u16::<Endian>(self.pants_hue)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CreateCharacterClassic(pub CreateCharacter);

impl Packet for CreateCharacterClassic {
    fn packet_kind() -> u8 { 0 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(0x86) }

    fn decode(client_version: ClientVersion, payload: &[u8]) -> anyhow::Result<Self> {
        CreateCharacter::decode(false, client_version, payload).map(Self)
    }

    fn encode(&self, client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        self.0.encode(false, client_version, writer)
    }
}

#[derive(Debug, Clone)]
pub struct CreateCharacterEnhanced(pub CreateCharacter);

impl Packet for CreateCharacterEnhanced {
    fn packet_kind() -> u8 { 0xf8 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(0x90) }

    fn decode(client_version: ClientVersion, payload: &[u8]) -> anyhow::Result<Self> {
        CreateCharacter::decode(true, client_version, payload).map(Self)
    }

    fn encode(&self, client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        self.0.encode(true, client_version, writer)
    }
}

#[derive(Debug, Clone)]
pub struct DeleteCharacter {
    pub character_index: u32,
    pub ip: u32,
}

impl Packet for DeleteCharacter {
    fn packet_kind() -> u8 { 0x83 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(38) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        payload.skip(30)?;
        let character_index = payload.read_u32::<Endian>()?;
        let ip = payload.read_u32::<Endian>()?;
        Ok(Self {
            character_index,
            ip,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_zeros(30)?;
        writer.write_u32::<Endian>(self.character_index)?;
        writer.write_u32::<Endian>(self.ip)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SelectCharacter {
    pub client_flags: ClientFlags,
    pub character_index: u32,
    pub name: String,
    pub ip: u32,
}

impl Packet for SelectCharacter {
    fn packet_kind() -> u8 { 0x5d }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(72) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        if payload.read_u32::<Endian>()? != CREATE_CHARACTER_MAGIC_1 {
            return Err(anyhow!("Invalid character select magic"));
        }

        let name = payload.read_str_block(30)?;
        payload.skip(2)?;
        let client_flags = ClientFlags::from_bits_truncate(payload.read_u32::<Endian>()?);
        payload.skip(24)?;
        let character_index = payload.read_u32::<Endian>()?;
        let ip = payload.read_u32::<Endian>()?;

        Ok(Self {
            client_flags,
            character_index,
            name,
            ip,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(CREATE_CHARACTER_MAGIC_1)?;
        writer.write_str_block(&self.name, 30)?;
        writer.write_zeros(2)?;
        writer.write_u32::<Endian>(self.client_flags.bits())?;
        writer.write_zeros(24)?;
        writer.write_u32::<Endian>(self.character_index)?;
        writer.write_u32::<Endian>(self.ip)?;
        Ok(())
    }
}