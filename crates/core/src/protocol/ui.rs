use std::io::{Read, Write};
use anyhow::anyhow;

use byteorder::{ReadBytesExt, WriteBytesExt};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::{ZlibEncoder};
use glam::IVec2;

use crate::EntityId;
use crate::protocol::{EntityFlags, PacketReadExt, PacketWriteExt};
use crate::protocol::client_version::VERSION_HIGH_SEAS;
use crate::protocol::format::utf16_slice_to_string;
use crate::types::FixedString;

use super::{ClientVersion, Packet, Endian};

#[derive(Debug, Clone)]
pub struct OpenChatWindow;

impl Packet for OpenChatWindow {
    fn packet_kind() -> u8 { 0xb5 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(64) }

    fn decode(_client_version: ClientVersion, _from_client: bool, _payload: &[u8]) -> anyhow::Result<Self> {
        Ok(Self)
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_zeros(63)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct OpenPaperDoll {
    pub id: EntityId,
    pub text: FixedString<60>,
    pub flags: EntityFlags,
}

impl Packet for OpenPaperDoll {
    fn packet_kind() -> u8 { 0x88 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(66) }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_entity_id()?;
        let text = payload.read_str_fixed()?;
        let flags = EntityFlags::from_bits_truncate(payload.read_u8()?);
        Ok(Self { id, text, flags })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.id)?;
        writer.write_str_fixed(&self.text)?;
        writer.write_u8(self.flags.bits())?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct OpenContainer {
    pub id: EntityId,
    pub gump_id: u16,
}

impl Packet for OpenContainer {
    fn packet_kind() -> u8 { 0x24 }

    fn fixed_length(client_version: ClientVersion) -> Option<usize> {
        Some(if client_version > VERSION_HIGH_SEAS { 9 } else { 7 })
    }

    fn decode(client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_entity_id()?;
        let gump_id = payload.read_u16::<Endian>()?;
        if client_version > VERSION_HIGH_SEAS {
            payload.skip(2)?;
        }
        Ok(Self { id, gump_id })
    }

    fn encode(&self, client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_entity_id(self.id)?;
        writer.write_u16::<Endian>(self.gump_id)?;
        if client_version > VERSION_HIGH_SEAS {
            writer.write_u16::<Endian>(0x7d)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GumpLayout {
    pub layout: String,
    pub text: Vec<String>,
}

impl TryFrom<CompressedGumpLayout> for GumpLayout {
    type Error = anyhow::Error;

    fn try_from(compressed: CompressedGumpLayout) -> anyhow::Result<Self> {
        let mut layout = String::with_capacity(compressed.layout_length);
        let mut reader = &compressed.layout[..];
        let mut z = ZlibDecoder::new(&mut reader);
        z.read_to_string(&mut layout)?;

        let mut reader = &compressed.text[..];
        let mut z = ZlibDecoder::new(&mut reader);
        let mut text = Vec::with_capacity(compressed.text_count);
        let mut tmp = Vec::new();
        for _ in 0..compressed.text_count {
            let len = z.read_u16::<Endian>()? as usize;
            tmp.resize(len * 2, 0u8);
            z.read_exact(&mut tmp)?;
            let line = utf16_slice_to_string(&tmp);
            text.push(line);
        }

        Ok(GumpLayout { layout, text })
    }
}

#[derive(Debug, Clone)]
pub struct OpenGump {
    pub id: u32,
    pub type_id: u32,
    pub position: IVec2,
    pub layout: GumpLayout,
}

impl Packet for OpenGump {
    fn packet_kind() -> u8 { 0xb0 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_u32::<Endian>()?;
        let type_id = payload.read_u32::<Endian>()?;
        let x = payload.read_i32::<Endian>()?;
        let y = payload.read_i32::<Endian>()?;

        let len = payload.read_u16::<Endian>()? as usize;
        let layout = std::str::from_utf8(&payload[..len])?.to_string();

        let text_count = payload.read_u16::<Endian>()? as usize;
        let mut text = Vec::with_capacity(text_count);
        for _ in 0..text_count {
            let line = payload.read_utf16_pascal()?;
            text.push(line);
        }

        let layout = GumpLayout {
            layout,
            text,
        };

        Ok(Self { id, type_id, position: IVec2::new(x, y), layout })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.id)?;
        writer.write_u32::<Endian>(self.type_id)?;
        writer.write_i32::<Endian>(self.position.x)?;
        writer.write_i32::<Endian>(self.position.y)?;

        writer.write_u16::<Endian>(self.layout.layout.len() as u16)?;
        writer.write_all(self.layout.layout.as_bytes())?;

        writer.write_u16::<Endian>(self.layout.text.len() as u16)?;
        for line in self.layout.text.iter() {
            writer.write_utf16_pascal(line)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CompressedGumpLayout {
    pub layout_length: usize,
    pub layout: Vec<u8>,
    pub text_count: usize,
    pub text_length: usize,
    pub text: Vec<u8>,
}

impl TryFrom<GumpLayout> for CompressedGumpLayout {
    type Error = anyhow::Error;

    fn try_from(source: GumpLayout) -> anyhow::Result<Self> {
        let mut layout = Vec::new();
        let mut z = ZlibEncoder::new(&mut layout, Compression::fast());
        z.write_all(source.layout.as_bytes())?;
        z.finish()?;

        let mut text = Vec::new();
        let mut z = ZlibEncoder::new(&mut text, Compression::fast());
        for line in source.text.iter() {
            z.write_utf16_pascal(line)?;
        }
        let text_length = z.total_in() as usize;
        z.finish()?;

        Ok(Self {
            layout_length: source.layout.len(),
            layout,
            text_count: source.text.len(),
            text_length,
            text,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OpenGumpCompressed {
    pub id: u32,
    pub type_id: u32,
    pub position: IVec2,
    pub layout: CompressedGumpLayout,
}

impl Packet for OpenGumpCompressed {
    fn packet_kind() -> u8 { 0xdd }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_u32::<Endian>()?;
        let type_id = payload.read_u32::<Endian>()?;
        let x = payload.read_i32::<Endian>()?;
        let y = payload.read_i32::<Endian>()?;

        let len_compressed = payload.read_u32::<Endian>()? as usize - 4;
        let layout_length = payload.read_u32::<Endian>()? as usize;
        if payload.len() < len_compressed {
            return Err(anyhow!("unexpected EOF"));
        }

        let layout = payload[..len_compressed].to_vec();
        payload.skip(len_compressed)?;

        let text_count = payload.read_u16::<Endian>()? as usize;

        let len_compressed = payload.read_u32::<Endian>()? as usize - 4;
        let text_length = payload.read_u32::<Endian>()? as usize;
        if payload.len() < len_compressed {
            return Err(anyhow!("unexpected EOF"));
        }

        let text = payload[..len_compressed].to_vec();
        payload.skip(len_compressed)?;

        let layout = CompressedGumpLayout {
            layout_length,
            layout,
            text_count,
            text_length,
            text,
        };

        Ok(Self { id, type_id, position: IVec2::new(x, y), layout })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.id)?;
        writer.write_u32::<Endian>(self.type_id)?;
        writer.write_i32::<Endian>(self.position.x)?;
        writer.write_i32::<Endian>(self.position.y)?;

        writer.write_u32::<Endian>(self.layout.layout.len() as u32 + 4)?;
        writer.write_u32::<Endian>(self.layout.layout_length as u32)?;
        writer.write_all(&self.layout.layout)?;

        writer.write_u16::<Endian>(self.layout.text_count as u16)?;
        writer.write_u32::<Endian>(self.layout.text.len() as u32 + 4)?;
        writer.write_u32::<Endian>(self.layout.text_length as u32)?;
        writer.write_all(&self.layout.text)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GumpResult {
    pub id: u32,
    pub type_id: u32,
    pub button_id: u32,
    pub on_switches: Vec<u32>,
    pub text_fields: Vec<String>,
}

impl Packet for GumpResult {
    fn packet_kind() -> u8 { 0xb1 }

    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { None }

    fn decode(_client_version: ClientVersion, _from_client: bool, mut payload: &[u8]) -> anyhow::Result<Self> {
        let id = payload.read_u32::<Endian>()?;
        let type_id = payload.read_u32::<Endian>()?;
        let button_id = payload.read_u32::<Endian>()?;
        let switch_count = payload.read_u32::<Endian>()? as usize;
        let mut on_switches = Vec::with_capacity(switch_count);
        for _ in 0..switch_count {
            on_switches.push(payload.read_u32::<Endian>()?);
        }

        let text_field_count = payload.read_u32::<Endian>()? as usize;
        let mut text_fields = Vec::with_capacity(text_field_count);
        for _ in 0..text_field_count {
            text_fields.push(payload.read_utf16_pascal()?);
        }

        Ok(Self { id, type_id, button_id, on_switches, text_fields })
    }

    fn encode(&self, _client_version: ClientVersion, _to_client: bool, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_u32::<Endian>(self.id)?;
        writer.write_u32::<Endian>(self.type_id)?;
        writer.write_u32::<Endian>(self.button_id)?;
        writer.write_u32::<Endian>(self.on_switches.len() as u32)?;
        for id in self.on_switches.iter() {
            writer.write_u32::<Endian>(*id)?;
        }
        writer.write_u32::<Endian>(self.text_fields.len() as u32)?;
        for content in self.text_fields.iter() {
            writer.write_utf16_pascal(content)?;
        }
        Ok(())
    }
}
