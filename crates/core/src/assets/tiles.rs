use std::io::Read;
use std::path::Path;

use bitflags::bitflags;
use byteorder::{LittleEndian as Endian, ReadBytesExt};

use crate::assets::mul::MulReader;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct TileFlags : u64 {
        const BACKGROUND = 1 << 0;
        const WEAPON = 1 << 1;
        const TRANSPARENT = 1 << 2;
        const TRANSLUCENT = 1 << 3;
        const WALL = 1 << 4;
        const DAMAGING = 1 << 5;
        const IMPASSABLE = 1 << 6;
        const WET = 1 << 7;
        const SURFACE = 1 << 9;
        const BRIDGE = 1 << 10;
        const FUNGIBLE = 1 << 11;
        const WINDOW = 1 << 12;
        const BLOCK_LOS = 1 << 13;
        const ARTICLE_A = 1 << 14;
        const ARTICLE_AN = 1 << 15;
        const INTERNAL = 1 << 16;
        const FOLIAGE = 1 << 17;
        const PARTIAL_HUE = 1 << 18;
        const MAP = 1 << 20;
        const CONTAINER = 1 << 21;
        const WEARABLE = 1 << 22;
        const LIGHT_SOURCE = 1 << 23;
        const ANIMATION = 1 << 24;
        const HOVER_OVER = 1 << 25;
        const ARMOUR = 1 << 27;
        const ROOF = 1 << 28;
        const DOOR = 1 << 29;
        const STAIR_BACK = 1 << 30;
        const STAIR_RIGHT = 1 << 31;
    }
}

#[derive(Debug, Clone)]
pub struct LandInfo {
    pub name: String,
    pub flags: TileFlags,
    pub texture_id: u16,
}

#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub name: String,
    pub flags: TileFlags,
    pub weight: u8,
    pub quality: u8,
    pub animation: u16,
    pub quantity: u8,
    pub value: u8,
    pub height: u8,
}

#[derive(Debug, Clone, Default)]
pub struct TileData {
    pub land: Vec<LandInfo>,
    pub items: Vec<ItemInfo>,
}

const NUM_LAND_TILES: usize = 0x4000;
const NUM_ITEMS: usize = 0x10000;

fn read_str_fixed(reader: &mut impl Read, len: usize) -> anyhow::Result<String> {
    let mut result = vec![0u8; len];
    reader.read_exact(&mut result)?;

    if let Some(n) = result.iter().position(|b| *b == 0) {
        result.truncate(n);
    }

    Ok(String::from_utf8(result)?)
}

pub async fn load_tile_data(data_path: &Path) -> anyhow::Result<TileData> {
    let mut reader = MulReader::open(data_path, "tiledata").await?;

    let mut land = Vec::with_capacity(NUM_LAND_TILES);
    let mut items = Vec::with_capacity(NUM_ITEMS);

    for index in 0..NUM_LAND_TILES {
        if index & 0x1f == 0 {
            reader.read_u32::<Endian>()?;
        }

        let flags = TileFlags::from_bits_truncate(reader.read_u64::<Endian>()?);
        let texture_id = reader.read_u16::<Endian>()?;
        let name = read_str_fixed(&mut reader, 20)?;
        land.push(LandInfo {
            name,
            flags,
            texture_id
        });
    }

    for index in 0..NUM_ITEMS {
        if index & 0x1f == 0 {
            reader.read_u32::<Endian>()?;
        }

        let flags = TileFlags::from_bits_truncate(reader.read_u64::<Endian>()?);
        let weight = reader.read_u8()?;
        let quality = reader.read_u8()?;
        let animation = reader.read_u16::<Endian>()?;
        reader.read_u8()?;
        let quantity = reader.read_u8()?;
        reader.read_u32::<Endian>()?;
        reader.read_u8()?;
        let value = reader.read_u8()?;
        let height = reader.read_u8()?;
        let name = read_str_fixed(&mut reader, 20)?;
        items.push(ItemInfo {
            name,
            flags,
            weight,
            quality,
            animation,
            quantity,
            value,
            height
        });
    }

    Ok(TileData { land, items })
}
