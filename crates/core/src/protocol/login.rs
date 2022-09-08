use std::io::Write;

use anyhow::anyhow;
use bitflags::bitflags;
use byteorder::{ReadBytesExt, WriteBytesExt};
use glam::UVec3;
use crate::Direction;

use crate::protocol::{PacketReadExt, PacketWriteExt};

use super::{ClientFlags, ClientVersion, Endian, Packet};

#[derive(Debug, Clone, Default)]
pub struct Seed {
    pub seed: u32,
    pub client_version: ClientVersion,
}

impl Packet for Seed {
    fn packet_kind() -> u8 { 0xef }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(21) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let seed = payload.read_u32::<Endian>()?;
        let major = payload.read_u32::<Endian>()? as u8;
        let minor = payload.read_u32::<Endian>()? as u8;
        let patch = payload.read_u32::<Endian>()? as u8;
        let build = payload.read_u32::<Endian>()? as u8;
        Ok(Self {
            seed,
            client_version: ClientVersion::new(major, minor, patch, build),
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.seed)?;
        writer.write_u32::<Endian>(self.client_version.major as u32)?;
        writer.write_u32::<Endian>(self.client_version.minor as u32)?;
        writer.write_u32::<Endian>(self.client_version.patch as u32)?;
        writer.write_u32::<Endian>(self.client_version.build as u32)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AccountLogin {
    pub username: String,
    pub password: String,
    pub next_login_key: u8,
}

impl Default for AccountLogin {
    fn default() -> Self {
        Self {
            username: Default::default(),
            password: Default::default(),
            next_login_key: 0xff,
        }
    }
}

impl Packet for AccountLogin {
    fn packet_kind() -> u8 { 0x80 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(62) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let username = payload.read_str_block(30)?;
        let password = payload.read_str_block(30)?;
        let next_login_key = payload.read_u8()?;

        Ok(AccountLogin {
            username,
            password,
            next_login_key,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_str_block(&self.username, 30)?;
        writer.write_str_block(&self.password, 30)?;
        writer.write_u8(self.next_login_key)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct GameServer {
    pub server_index: u16,
    pub server_name: String,
    pub load_percent: u8,
    pub timezone: u8,
    pub ip: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ServerList {
    pub system_info_flags: u8,
    pub game_servers: Vec<GameServer>,
}

impl Packet for ServerList {
    fn packet_kind() -> u8 { 0xa8 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let system_info_flags = payload.read_u8()?;
        let game_server_count = payload.read_u16::<Endian>()? as usize;
        let mut game_servers = Vec::with_capacity(game_server_count);

        for _ in 0..game_server_count {
            let server_index = payload.read_u16::<Endian>()?;
            let server_name = payload.read_str_block(32)?;
            let load_percent = payload.read_u8()?;
            let timezone = payload.read_u8()?;
            let ip = payload.read_u32::<Endian>()?;
            game_servers.push(GameServer {
                server_index,
                server_name,
                load_percent,
                timezone,
                ip,
            });
        }

        Ok(Self {
            system_info_flags,
            game_servers,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(self.system_info_flags)?;
        writer.write_u16::<Endian>(self.game_servers.len() as u16)?;

        for server in self.game_servers.iter() {
            writer.write_u16::<Endian>(server.server_index)?;
            writer.write_str_block(&server.server_name, 32)?;
            writer.write_u8(server.load_percent)?;
            writer.write_u8(server.timezone)?;
            writer.write_u32::<Endian>(server.ip)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SelectGameServer {
    server_id: u8,
}

impl Packet for SelectGameServer {
    fn packet_kind() -> u8 { 0xA0 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(3) }

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

#[derive(Debug, Clone, Default)]
pub struct SwitchServer {
    pub ip: u32,
    pub port: u16,
    pub token: u32,
}

impl Packet for SwitchServer {
    fn packet_kind() -> u8 { 0x8c }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(11) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let ip = payload.read_u32::<Endian>()?;
        let port = payload.read_u16::<Endian>()?;
        let token = payload.read_u32::<Endian>()?;
        Ok(Self { ip, port, token })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.ip)?;
        writer.write_u16::<Endian>(self.port)?;
        writer.write_u32::<Endian>(self.token)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GameServerLogin {
    pub token: u32,
    pub username: String,
    pub password: String,
}

impl Packet for GameServerLogin {
    fn packet_kind() -> u8 { 0x91 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(0x41) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let token = payload.read_u32::<Endian>()?;
        let username = payload.read_str_block(30)?;
        let password = payload.read_str_block(30)?;
        Ok(GameServerLogin {
            token,
            username,
            password,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.token)?;
        writer.write_str_block(&self.username, 30)?;
        writer.write_str_block(&self.password, 30)?;
        Ok(())
    }
}

bitflags! {
    #[derive(Default)]
    pub struct FeatureFlags : u32 {
        const T2A = 0x1;
        const UOR = 0x2;
        const UOTD = 0x4;
        const LBR = 0x8;
        const AOS = 0x10;
        const SIXTH_CHARACTER_SLOT = 0x20;
        const SE = 0x40;
        const ML = 0x80;
        const EIGTH_AGE = 0x100;
        const NINTH_AGE = 0x200;
        const TENTH_AGE = 0x400;
        const INCREASED_STORAGE = 0x800;
        const SEVENTH_CHARACTER_SLOT = 0x1000;
        const ROLEPLAY_FACES = 0x2000;
        const TRIAL_ACCOUNT = 0x4000;
        const LIVE_ACCOUNT = 0x8000;
        const SA = 0x10000;
        const HS = 0x20000;
        const GOTHIC = 0x40000;
        const RUSTIC = 0x80000;
        const JUNGLE = 0x100000;
        const SHADOWGUARD = 0x200000;
        const TOL = 0x400000;
        const EJ = 0x800000;
    }
}

#[derive(Debug, Clone, Default)]
pub struct SupportedFeatures {
    pub feature_flags: FeatureFlags,
}

impl SupportedFeatures {
    const EXTENDED_MIN_VERSION: ClientVersion = ClientVersion::new(6, 0, 14, 2);
}

impl Packet for SupportedFeatures {
    fn packet_kind() -> u8 { 0xb9 }

    fn fixed_length(client_version: ClientVersion) -> Option<usize> {
        Some(if client_version >= Self::EXTENDED_MIN_VERSION { 5 } else { 3 })
    }

    fn decode(client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let extended = client_version >= Self::EXTENDED_MIN_VERSION;
        let feature_flags = FeatureFlags::from_bits_truncate(if extended {
            payload.read_u32::<Endian>()?
        } else {
            payload.read_u16::<Endian>()? as u32
        });
        Ok(SupportedFeatures {
            feature_flags,
        })
    }

    fn encode(&self, client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        let extended = client_version >= Self::EXTENDED_MIN_VERSION;
        if extended {
            writer.write_u32::<Endian>(self.feature_flags.bits())?;
        } else {
            writer.write_u16::<Endian>(self.feature_flags.bits() as u16)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct CharacterFromList {
    pub name: String,
    pub password: String,
}

#[derive(Debug, Clone, Default)]
pub struct StartingCity {
    pub index: u8,
    pub city: String,
    pub building: String,
    pub location: UVec3,
    pub map_id: u32,
    pub description_id: u32,
}

bitflags! {
    pub struct CharacterListFlags : u32 {
        const SINGLE_CHARACTER_SLOT = 0x4;
        const SLOT_LIMIT = 0x10;
        const SIXTH_CHARACTER_SLOT = 0x40;
        const SEVENTH_CHARACTER_SLOT = 0x1000;
    }
}

#[derive(Debug, Clone, Default)]
pub struct CharacterList {
    pub characters: Vec<Option<CharacterFromList>>,
    pub cities: Vec<StartingCity>,
}

impl CharacterList {
    const NEW_CHARACTER_LIST: ClientVersion = ClientVersion::new(7, 0, 13, 0);
}

impl Packet for CharacterList {
    fn packet_kind() -> u8 { 0xa9 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let new_character_list = client_version >= Self::NEW_CHARACTER_LIST;
        let text_length = if new_character_list { 32 } else { 31 };

        let slot_count = payload.read_u8()? as usize;
        let mut characters = Vec::with_capacity(slot_count);

        for _ in 0..slot_count {
            let name = payload.read_str_block(30)?;
            let password = payload.read_str_block(30)?;

            characters.push(if !name.is_empty() {
                Some(CharacterFromList {
                    name,
                    password,
                })
            } else {
                None
            });
        }

        let city_count = payload.read_u8()? as usize;
        let mut cities = Vec::with_capacity(city_count);

        for _ in 0..city_count {
            let index = payload.read_u8()?;
            let city = payload.read_str_block(text_length)?;
            let building = payload.read_str_block(text_length)?;

            let (location, map_id, description_id) = if new_character_list {
                let x = payload.read_u32::<Endian>()?;
                let y = payload.read_u32::<Endian>()?;
                let z = payload.read_u32::<Endian>()?;
                let map_id = payload.read_u32::<Endian>()?;
                let description_id = payload.read_u32::<Endian>()?;
                payload.skip(4)?;
                (UVec3::new(x, y, z), map_id, description_id)
            } else {
                (UVec3::new(0, 0, 0), 0, 0)
            };

            cities.push(StartingCity {
                index,
                city,
                building,
                location,
                map_id,
                description_id,
            });
        }

        Ok(CharacterList {
            characters,
            cities,
        })
    }

    fn encode(&self, client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        let new_character_list = client_version >= Self::NEW_CHARACTER_LIST;
        let text_length = if new_character_list { 32 } else { 31 };

        writer.write_u8(self.characters.len() as u8)?;
        for character in self.characters.iter() {
            if let Some(character) = character {
                writer.write_str_block(&character.name, 30)?;
                writer.write_str_block(&character.password, 30)?;
            } else {
                writer.write_zeros(60)?;
            }
        }

        writer.write_u8(self.cities.len() as u8)?;
        for city in self.cities.iter() {
            writer.write_u8(city.index)?;
            writer.write_str_block(&city.city, text_length)?;
            writer.write_str_block(&city.building, text_length)?;

            if new_character_list {
                writer.write_u32::<Endian>(city.location.x)?;
                writer.write_u32::<Endian>(city.location.y)?;
                writer.write_u32::<Endian>(city.location.z)?;
                writer.write_u32::<Endian>(city.map_id)?;
                writer.write_u32::<Endian>(city.description_id)?;
                writer.write_u32::<Endian>(0)?;
            }
        }

        let mut flags = CharacterListFlags::empty();

        if self.characters.len() > 6 {
            flags |= CharacterListFlags::SEVENTH_CHARACTER_SLOT;
        }

        if self.characters.len() > 5 {
            flags |= CharacterListFlags::SIXTH_CHARACTER_SLOT;
        }

        if self.characters.len() == 1 {
            flags |= CharacterListFlags::SLOT_LIMIT
                | CharacterListFlags::SINGLE_CHARACTER_SLOT;
        }

        writer.write_u32::<Endian>(flags.bits())?;
        if new_character_list {
            writer.write_u16::<Endian>(0xffff)?;
        }

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
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(104) }

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
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(106) }

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

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(39) }

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

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(73) }

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

#[derive(Debug, Clone, Default)]
pub struct ClientVersionRequest {
    pub version: String,
}

impl Packet for ClientVersionRequest {
    fn packet_kind() -> u8 { 0xbd }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        Ok(ClientVersionRequest {
            version: payload.read_str_nul()?,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_str_nul(&self.version)
    }
}

#[derive(Debug, Clone, Default)]
pub struct BeginEnterWorld {
    pub mobile_id: u32,
    pub body: u16,
    pub position: UVec3,
    pub direction: Direction,
    pub map_width: u16,
    pub map_height: u16,
}

impl Packet for BeginEnterWorld {
    fn packet_kind() -> u8 { 0x1b }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(37) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let mobile_id = payload.read_u32::<Endian>()?;
        payload.skip(4)?;
        let body = payload.read_u16::<Endian>()?;
        let x = payload.read_u16::<Endian>()? as u32;
        let y = payload.read_u16::<Endian>()? as u32;
        let z = payload.read_u16::<Endian>()? as u32;
        let direction = Direction::from_u8(payload.read_u8()?)
            .ok_or_else(|| anyhow!("Invalid direction"))?;
        payload.skip(9)?;
        let map_width = payload.read_u16::<Endian>()?;
        let map_height = payload.read_u16::<Endian>()?;
        Ok(BeginEnterWorld {
            mobile_id,
            body,
            position: UVec3::new(x, y, z),
            direction,
            map_width,
            map_height,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.mobile_id)?;
        writer.write_u32::<Endian>(0)?;
        writer.write_u16::<Endian>(self.body)?;
        writer.write_i16::<Endian>(self.position.x as i16)?;
        writer.write_i16::<Endian>(self.position.y as i16)?;
        writer.write_i16::<Endian>(self.position.z as i16)?;
        writer.write_u8(self.direction as u8)?;
        writer.write_u8(0)?;
        writer.write_u32::<Endian>(0xffffffff)?;
        writer.write_u32::<Endian>(0)?;
        writer.write_u16::<Endian>(self.map_width)?;
        writer.write_u16::<Endian>(self.map_height)?;
        writer.write_zeros(6)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct EndEnterWorld;

impl Packet for EndEnterWorld {
    fn packet_kind() -> u8 { 0x55 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(1) }

    fn decode(_client_version: ClientVersion, _payload: &[u8]) -> anyhow::Result<Self> {
        Ok(EndEnterWorld)
    }

    fn encode(&self, _client_version: ClientVersion, _writer: &mut impl Write) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ShowPublicHouses {
    pub show: bool,
}

impl Packet for ShowPublicHouses {
    fn packet_kind() -> u8 { 0xfb }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(2) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let show = payload.read_u8()? != 0;
        Ok(Self { show })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u8(if self.show { 1 } else { 0 })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Ping;

impl Packet for Ping {
    fn packet_kind() -> u8 { 0x73 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(2) }

    fn decode(_client_version: ClientVersion, _payload: &[u8]) -> anyhow::Result<Self> {
        Ok(Self)
    }

    fn encode(&self, _client_version: ClientVersion, _writer: &mut impl Write) -> anyhow::Result<()> {
        Ok(())
    }
}