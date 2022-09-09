use std::io::Write;

use anyhow::anyhow;
use bitflags::bitflags;
use byteorder::{ReadBytesExt, WriteBytesExt};
use glam::IVec3;

use crate::{Direction, EntityId, EntityKind, Notoriety};
use crate::protocol::PacketWriteExt;

use super::{ClientVersion, Endian, Packet, PacketReadExt};

pub const REQUEST_MOBILE_STATUS: u8 = 4;
pub const REQUEST_MOBILE_SKILLS: u8 = 4;

#[derive(Debug, Clone)]
pub struct EntityRequest {
    pub kind: u8,
    pub target: EntityId,
}

impl Packet for EntityRequest {
    fn packet_kind() -> u8 { 0x34 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(10) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        payload.skip(4)?;
        let kind = payload.read_u8()?;
        let target = payload.read_entity_id()?;
        Ok(EntityRequest { kind, target })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(0xedededed)?;
        writer.write_u8(self.kind)?;
        writer.write_entity_id(self.target)?;
        Ok(())
    }
}

bitflags! {
    #[derive(Default)]
    pub struct EntityFlags : u8 {
        const FEMALE = 0x2;
        const POISONED = 0x4;
        const YELLOW_HITS = 0x8;
        const FACTION_SHIP = 0x10;
        const MOVABLE = 0x20;
        const WAR_MODE = 0x40;
        const HIDDEN = 0x80;
    }
}

#[derive(Debug, Clone, Default)]
pub struct UpsertEntityLegacy {
    pub id: EntityId,
    pub graphic_id: u32,
    pub quantity: u16,
    pub position: IVec3,
    pub direction: Direction,
    pub dye: u16,
    pub flags: EntityFlags,
}

impl Packet for UpsertEntityLegacy {
    fn packet_kind() -> u8 { 0x1a }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_entity_id()?.as_u32();
        let mut graphic_id = payload.read_u16::<Endian>()? as u32;
        let quantity = if id & 0x80000000 != 0 {
            payload.read_u16::<Endian>()?
        } else {
            0
        };

        if graphic_id & 0x8000 != 0 {
            graphic_id += payload.read_u8()? as u32;
        }

        let x = payload.read_u16::<Endian>()? as i32;
        let y = payload.read_u16::<Endian>()? as i32;
        let direction = if x & 0x8000 != 0 {
            payload.read_direction()?
        } else {
            Direction::North
        };
        let z = payload.read_u8()? as i32;
        let dye = if y & 0x8000 != 0 {
            payload.read_u16::<Endian>()?
        } else {
            0
        };
        let flags = if y & 0x4000 != 0 {
            EntityFlags::from_bits_truncate(payload.read_u8()?)
        } else {
            EntityFlags::empty()
        };

        Ok(Self {
            id: EntityId::from_u32(id & 0x7fffffff),
            graphic_id,
            quantity,
            position: IVec3::new(x & 0x3fff, y & 0x3fff, z),
            direction,
            dye,
            flags,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.id.as_u32() | if self.quantity > 1 {
            0x80000000
        } else {
            0
        })?;

        writer.write_u16::<Endian>((self.graphic_id & 0x7fff) as u16)?;

        if self.quantity > 1 {
            writer.write_u16::<Endian>(self.quantity)?;
        }

        let mut x = self.position.x as u16;
        if self.direction != Direction::North {
            x |= 0x8000;
        }

        let mut y = self.position.y as u16;

        if self.dye != 0 {
            y |= 0x8000;
        }

        if !self.flags.is_empty() {
            y |= 0x4000;
        }

        writer.write_u16::<Endian>(x)?;
        writer.write_u16::<Endian>(y)?;

        if self.direction != Direction::North {
            writer.write_direction(self.direction)?;
        }

        writer.write_u8(self.position.z as u8)?;

        if self.dye != 0 {
            writer.write_u16::<Endian>(self.dye)?;
        }

        if !self.flags.is_empty() {
            writer.write_u8(self.flags.bits())?;
        }

        Ok(())
    }
}


#[derive(Debug, Clone, Default)]
pub struct UpsertEntity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub graphic_id: u16,
    pub direction: Direction,
    pub quantity: u16,
    pub position: IVec3,
    pub layer: u8,
    pub hue: u16,
    pub flags: EntityFlags,
}

impl Packet for UpsertEntity {
    fn packet_kind() -> u8 { 0xf3 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(24) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        payload.skip(2)?;
        let kind = EntityKind::from_u8(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid entity kind"))?;
        let id = payload.read_entity_id()?;
        let graphic_id = payload.read_u16::<Endian>()?;
        let direction = payload.read_direction()?;
        let quantity = payload.read_u16::<Endian>()?;
        payload.skip(2)?;
        let x = payload.read_u16::<Endian>()? as i32;
        let y = payload.read_u16::<Endian>()? as i32;
        let z = payload.read_u8()? as i32;
        let layer = payload.read_u8()?;
        let hue = payload.read_u16::<Endian>()?;
        let flags = EntityFlags::from_bits_truncate(payload.read_u8()?);
        Ok(Self {
            id,
            kind,
            graphic_id,
            direction,
            quantity,
            position: IVec3::new(x, y, z),
            layer,
            hue,
            flags,
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u16::<Endian>(1)?;
        writer.write_u8(self.kind as u8)?;
        writer.write_entity_id(self.id)?;
        writer.write_u16::<Endian>(self.graphic_id)?;
        writer.write_direction(self.direction)?;
        writer.write_u16::<Endian>(self.position.x as u16)?;
        writer.write_u16::<Endian>(self.position.y as u16)?;
        writer.write_u8(self.position.z as u8)?;
        writer.write_u8(self.layer)?;
        writer.write_u16::<Endian>(self.hue)?;
        writer.write_u8(self.flags.bits())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeleteEntity {
    pub id: EntityId,
}

impl Packet for DeleteEntity {
    fn packet_kind() -> u8 { 0x1d }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(5) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_entity_id()?;
        Ok(Self { id })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.id)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct UpsertLocalPlayer {
    pub id: EntityId,
    pub body_type: u16,
    pub hue: u16,
    pub server_id: u16,
    pub flags: EntityFlags,
    pub position: IVec3,
    pub direction: Direction,
}

impl Packet for UpsertLocalPlayer {
    fn packet_kind() -> u8 { 0x20 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(19) }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_entity_id()?;
        let body_type = payload.read_u16::<Endian>()?;
        payload.skip(1)?;
        let hue = payload.read_u16::<Endian>()?;
        let flags = EntityFlags::from_bits_truncate(payload.read_u8()?);
        let x = payload.read_u16::<Endian>()? as i32;
        let y = payload.read_u16::<Endian>()? as i32;
        let server_id = payload.read_u16::<Endian>()?;
        let direction = payload.read_direction()?;
        let z = payload.read_u8()? as i32;
        Ok(Self {
            id,
            body_type,
            hue,
            server_id,
            flags,
            position: IVec3::new(x, y, z),
            direction
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.id)?;
        writer.write_u16::<Endian>(self.body_type)?;
        writer.write_u8(0)?;
        writer.write_u16::<Endian>(self.hue)?;
        writer.write_u8(self.flags.bits())?;
        writer.write_u16::<Endian>(self.position.x as u16)?;
        writer.write_u16::<Endian>(self.position.y as u16)?;
        writer.write_u16::<Endian>(self.server_id)?;
        writer.write_direction(self.direction)?;
        writer.write_u8(self.position.z as u8)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct CharacterChildEntity {
    pub id: EntityId,
    pub graphic_id: u16,
    pub layer: u8,
    pub hue: u16,
}

#[derive(Debug, Clone, Default)]
pub struct UpsertEntityCharacter {
    pub id: EntityId,
    pub body_type: u16,
    pub position: IVec3,
    pub direction: Direction,
    pub hue: u16,
    pub flags: EntityFlags,
    pub notoriety: Notoriety,
    pub children: Vec<CharacterChildEntity>,
}

impl Packet for UpsertEntityCharacter {
    fn packet_kind() -> u8 { 0x78 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_entity_id()?;
        let graphic_id = payload.read_u16::<Endian>()?;
        let x = payload.read_u16::<Endian>()? as i32;
        let y = payload.read_u16::<Endian>()? as i32;
        let z = payload.read_u8()? as i32;
        let direction = payload.read_direction()?;
        let hue = payload.read_u16::<Endian>()?;
        let flags = EntityFlags::from_bits_truncate(payload.read_u8()?);
        let notoriety = Notoriety::from_u8(payload.read_u8()?)
            .ok_or_else(|| anyhow!("invalid notoriety"))?;
        let mut children = Vec::new();

        loop {
            let child_id = payload.read_entity_id()?;
            if !child_id.is_valid() {
                break;
            }

            let graphic_id = payload.read_u16::<Endian>()?;
            let layer = payload.read_u8()?;
            let hue = if graphic_id & 0x8000 != 0 {
                payload.read_u16::<Endian>()?
            } else {
                0
            };
            children.push(CharacterChildEntity {
                id: child_id,
                graphic_id,
                layer,
                hue,
            });
        }

        Ok(Self {
            id,
            body_type: graphic_id,
            position: IVec3::new(x, y, z),
            direction,
            hue,
            flags,
            notoriety,
            children
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.id)?;
        writer.write_u16::<Endian>(self.body_type)?;
        writer.write_u16::<Endian>(self.position.x as u16)?;
        writer.write_u16::<Endian>(self.position.y as u16)?;
        writer.write_u8(self.position.z as u8)?;
        writer.write_direction(self.direction)?;
        writer.write_u16::<Endian>(self.hue)?;
        writer.write_u8(self.flags.bits())?;
        writer.write_u8(self.notoriety as u8)?;

        for child in self.children.iter() {
            writer.write_entity_id(child.id)?;
            writer.write_u16::<Endian>(child.graphic_id | if child.hue != 0 {
                0x8000
            } else {
                0
            })?;
            if child.hue != 0 {
                writer.write_u16::<Endian>(child.hue)?;
            }
        }

        writer.write_entity_id(EntityId::ZERO)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct UpsertEntityStats {
    pub id: EntityId,
    pub max_info_level: u8,
    pub name: String,
    pub allow_name_change: bool,
    pub race_and_gender: u8,
    pub hp: u16,
    pub max_hp: u16,
    pub str: u16,
    pub dex: u16,
    pub int: u16,
    pub stamina: u16,
    pub max_stamina: u16,
    pub mana: u16,
    pub max_mana: u16,
    pub gold: u32,
    pub armor: u16,
    pub weight: u16,
    pub max_weight: u16,
    pub stats_cap: u16,
    pub pet_count: u8,
    pub max_pets: u8,
    pub fire_resist: u16,
    pub cold_resist: u16,
    pub poison_resist: u16,
    pub energy_resist: u16,
    pub luck: u16,
    pub damage_min: u16,
    pub damage_max: u16,
    pub tithing: u32,
    pub hit_chance_bonus: u16,
    pub swing_speed_bonus: u16,
    pub damage_chance_bonus: u16,
    pub reagent_cost_bonus: u16,
    pub hp_regen: u16,
    pub stamina_regen: u16,
    pub mana_regen: u16,
    pub damage_reflect: u16,
    pub potion_bonus: u16,
    pub defence_chance_bonus: u16,
    pub spell_damage_bonus: u16,
    pub cooldown_bonus: u16,
    pub cast_time_bonus: u16,
    pub mana_cost_bonus: u16,
    pub str_bonus: u16,
    pub dex_bonus: u16,
    pub int_bonus: u16,
    pub hp_bonus: u16,
    pub stamina_bonus: u16,
    pub mana_bonus: u16,
    pub max_hp_bonus: u16,
    pub max_stamina_bonus: u16,
    pub max_mana_bonus: u16,
}

impl Packet for UpsertEntityStats {
    fn packet_kind() -> u8 { 0x11 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_entity_id()?;
        let name = payload.read_str_block(30)?;
        let hp = payload.read_u16::<Endian>()?;
        let max_hp = payload.read_u16::<Endian>()?;
        let allow_name_change = payload.read_u8()? != 0;
        let level = payload.read_u8()?;

        if level == 0 {
            return Ok(Self {
                id,
                name,
                hp,
                max_hp,
                allow_name_change,
                ..Default::default()
            })
        }

        let race_and_gender = payload.read_u8()?;
        let str = payload.read_u16::<Endian>()?;
        let dex = payload.read_u16::<Endian>()?;
        let int = payload.read_u16::<Endian>()?;
        let stamina = payload.read_u16::<Endian>()?;
        let max_stamina = payload.read_u16::<Endian>()?;
        let mana = payload.read_u16::<Endian>()?;
        let max_mana = payload.read_u16::<Endian>()?;
        let gold = payload.read_u32::<Endian>()?;
        let armor = payload.read_u16::<Endian>()?;
        let weight = payload.read_u16::<Endian>()?;
        let max_weight = payload.read_u16::<Endian>()?;
        payload.skip(1)?;
        let stats_cap = payload.read_u16::<Endian>()?;
        let pet_count = payload.read_u8()?;
        let max_pets = payload.read_u8()?;
        let fire_resist = payload.read_u16::<Endian>()?;
        let cold_resist = payload.read_u16::<Endian>()?;
        let poison_resist = payload.read_u16::<Endian>()?;
        let energy_resist = payload.read_u16::<Endian>()?;
        let luck = payload.read_u16::<Endian>()?;
        let damage_min = payload.read_u16::<Endian>()?;
        let damage_max = payload.read_u16::<Endian>()?;
        let tithing = payload.read_u32::<Endian>()?;
        let hit_chance_bonus = payload.read_u16::<Endian>()?;
        let swing_speed_bonus = payload.read_u16::<Endian>()?;
        let damage_chance_bonus = payload.read_u16::<Endian>()?;
        let reagent_cost_bonus = payload.read_u16::<Endian>()?;
        let hp_regen = payload.read_u16::<Endian>()?;
        let stamina_regen = payload.read_u16::<Endian>()?;
        let mana_regen = payload.read_u16::<Endian>()?;
        let damage_reflect = payload.read_u16::<Endian>()?;
        let potion_bonus = payload.read_u16::<Endian>()?;
        let defence_chance_bonus = payload.read_u16::<Endian>()?;
        let spell_damage_bonus = payload.read_u16::<Endian>()?;
        let cooldown_bonus = payload.read_u16::<Endian>()?;
        let cast_time_bonus = payload.read_u16::<Endian>()?;
        let mana_cost_bonus = payload.read_u16::<Endian>()?;
        let str_bonus = payload.read_u16::<Endian>()?;
        let dex_bonus = payload.read_u16::<Endian>()?;
        let int_bonus = payload.read_u16::<Endian>()?;
        let hp_bonus = payload.read_u16::<Endian>()?;
        let stamina_bonus = payload.read_u16::<Endian>()?;
        let mana_bonus = payload.read_u16::<Endian>()?;
        let max_hp_bonus = payload.read_u16::<Endian>()?;
        let max_stamina_bonus = payload.read_u16::<Endian>()?;
        let max_mana_bonus = payload.read_u16::<Endian>()?;

        Ok(Self {
            id,
            max_info_level: level,
            name,
            allow_name_change,
            race_and_gender,
            hp,
            max_hp,
            str,
            dex,
            int,
            stamina,
            max_stamina,
            mana,
            max_mana,
            gold,
            armor,
            weight,
            max_weight,
            stats_cap,
            pet_count,
            max_pets,
            fire_resist,
            cold_resist,
            poison_resist,
            energy_resist,
            luck,
            damage_min,
            damage_max,
            tithing,
            hit_chance_bonus,
            swing_speed_bonus,
            damage_chance_bonus,
            reagent_cost_bonus,
            hp_regen,
            stamina_regen,
            mana_regen,
            damage_reflect,
            potion_bonus,
            defence_chance_bonus,
            spell_damage_bonus,
            cooldown_bonus,
            cast_time_bonus,
            mana_cost_bonus,
            str_bonus,
            dex_bonus,
            int_bonus,
            hp_bonus,
            stamina_bonus,
            mana_bonus,
            max_hp_bonus,
            max_stamina_bonus,
            max_mana_bonus
        })
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        let level = self.max_info_level;

        writer.write_entity_id(self.id)?;
        writer.write_str_block(&self.name, 30)?;
        writer.write_u16::<Endian>(self.hp)?;
        writer.write_u16::<Endian>(self.max_hp)?;
        writer.write_u8(if self.allow_name_change { 1 } else { 0 })?;
        writer.write_u8(level)?;

        // TODO: in order to support older clients, this needs to be more granular
        if level > 0 {
            writer.write_u8(self.race_and_gender)?;
            writer.write_u16::<Endian>(self.str)?;
            writer.write_u16::<Endian>(self.dex)?;
            writer.write_u16::<Endian>(self.int)?;
            writer.write_u16::<Endian>(self.stamina)?;
            writer.write_u16::<Endian>(self.max_stamina)?;
            writer.write_u16::<Endian>(self.mana)?;
            writer.write_u16::<Endian>(self.max_mana)?;
            writer.write_u32::<Endian>(self.gold)?;
            writer.write_u16::<Endian>(self.armor)?;
            writer.write_u16::<Endian>(self.weight)?;
            writer.write_u16::<Endian>(self.max_weight)?;
            writer.write_u8((self.race_and_gender >> 1) + 1)?;
            writer.write_u16::<Endian>(self.stats_cap)?;
            writer.write_u8(self.pet_count)?;
            writer.write_u8(self.max_pets)?;
            writer.write_u16::<Endian>(self.fire_resist)?;
            writer.write_u16::<Endian>(self.cold_resist)?;
            writer.write_u16::<Endian>(self.poison_resist)?;
            writer.write_u16::<Endian>(self.energy_resist)?;
            writer.write_u16::<Endian>(self.luck)?;
            writer.write_u16::<Endian>(self.damage_min)?;
            writer.write_u16::<Endian>(self.damage_max)?;
            writer.write_u32::<Endian>(self.tithing)?;
            writer.write_u16::<Endian>(self.hit_chance_bonus)?;
            writer.write_u16::<Endian>(self.swing_speed_bonus)?;
            writer.write_u16::<Endian>(self.damage_chance_bonus)?;
            writer.write_u16::<Endian>(self.reagent_cost_bonus)?;
            writer.write_u16::<Endian>(self.hp_regen)?;
            writer.write_u16::<Endian>(self.stamina_regen)?;
            writer.write_u16::<Endian>(self.mana_regen)?;
            writer.write_u16::<Endian>(self.damage_reflect)?;
            writer.write_u16::<Endian>(self.potion_bonus)?;
            writer.write_u16::<Endian>(self.defence_chance_bonus)?;
            writer.write_u16::<Endian>(self.spell_damage_bonus)?;
            writer.write_u16::<Endian>(self.cooldown_bonus)?;
            writer.write_u16::<Endian>(self.cast_time_bonus)?;
            writer.write_u16::<Endian>(self.mana_cost_bonus)?;
            writer.write_u16::<Endian>(self.str_bonus)?;
            writer.write_u16::<Endian>(self.dex_bonus)?;
            writer.write_u16::<Endian>(self.int_bonus)?;
            writer.write_u16::<Endian>(self.hp_bonus)?;
            writer.write_u16::<Endian>(self.stamina_bonus)?;
            writer.write_u16::<Endian>(self.mana_bonus)?;
            writer.write_u16::<Endian>(self.max_hp_bonus)?;
            writer.write_u16::<Endian>(self.max_stamina_bonus)?;
            writer.write_u16::<Endian>(self.max_mana_bonus)?;
        }

        Ok(())
    }
}
