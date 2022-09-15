use std::io::Write;

use anyhow::anyhow;
use byteorder::{ByteOrder, LE, ReadBytesExt, WriteBytesExt};
use encode_unicode::{Utf16Char, IterExt};

use crate::{Direction, EntityId};

use super::Endian;

static ZEROS: [u8; 1024] = [0u8; 1024];

pub trait PacketWriteExt {
    fn write_zeros(&mut self, count: usize) -> anyhow::Result<()>;
    fn write_str_block(&mut self, src: &str, block_size: usize) -> anyhow::Result<()>;
    fn write_str_nul(&mut self, src: &str) -> anyhow::Result<()>;
    fn write_utf16_nul(&mut self, src: &str) -> anyhow::Result<()>;
    fn write_utf16_pascal(&mut self, src: &str) -> anyhow::Result<()>;
    fn write_utf16le_pascal(&mut self, src: &str) -> anyhow::Result<()>;
    fn write_entity_id(&mut self, src: EntityId) -> anyhow::Result<()>;
    fn write_direction(&mut self, src: Direction) -> anyhow::Result<()>;
}

impl<T: Write> PacketWriteExt for T {
    fn write_zeros(&mut self, mut count: usize) -> anyhow::Result<()> {
        while count > 0 {
            let to_write = count.min(ZEROS.len());
            self.write_all(&ZEROS[..to_write])?;
            count -= to_write;
        }

        Ok(())
    }

    fn write_str_block(&mut self, src: &str, block_size: usize) -> anyhow::Result<()> {
        if src.len() >= block_size {
            Ok(self.write_all(&src.as_bytes()[..block_size])?)
        } else {
            self.write_all(src.as_bytes())?;
            self.write_zeros(block_size - src.len())
        }
    }

    fn write_str_nul(&mut self, src: &str) -> anyhow::Result<()> {
        self.write_all(src.as_bytes())?;
        self.write_u8(0)?;
        Ok(())
    }

    fn write_utf16_nul(&mut self, src: &str) -> anyhow::Result<()> {
        for c in src.encode_utf16() {
            self.write_u16::<Endian>(c)?;
        }
        Ok(())
    }

    fn write_utf16_pascal(&mut self, src: &str) -> anyhow::Result<()> {
        let len = src.encode_utf16().count();
        self.write_u16::<Endian>(len as u16)?;
        for c in src.encode_utf16() {
            self.write_u16::<Endian>(c)?;
        }
        Ok(())
    }

    fn write_utf16le_pascal(&mut self, src: &str) -> anyhow::Result<()> {
        let len = src.encode_utf16().count();
        self.write_u16::<LE>(len as u16)?;
        for c in src.encode_utf16() {
            self.write_u16::<LE>(c)?;
        }
        Ok(())
    }

    fn write_entity_id(&mut self, src: EntityId) -> anyhow::Result<()> {
        Ok(self.write_u32::<Endian>(src.as_u32())?)
    }

    fn write_direction(&mut self, src: Direction) -> anyhow::Result<()> {
        Ok(self.write_u8(src as u8)?)
    }
}

pub trait PacketReadExt {
    fn skip(&mut self, count: usize) -> anyhow::Result<()>;
    fn read_str_block(&mut self, block_size: usize) -> anyhow::Result<String>;
    fn read_str_nul(&mut self) -> anyhow::Result<String>;
    fn read_utf16_nul(&mut self) -> anyhow::Result<String>;
    fn read_utf16_pascal(&mut self) -> anyhow::Result<String>;
    fn read_utf16le_pascal(&mut self) -> anyhow::Result<String>;
    fn read_entity_id(&mut self) -> anyhow::Result<EntityId>;
    fn read_direction(&mut self) -> anyhow::Result<Direction>;
}

impl PacketReadExt for &[u8] {
    fn skip(&mut self, count: usize) -> anyhow::Result<()> {
        if count > self.len() {
            Err(anyhow!("unexpected EOF"))
        } else {
            *self = &self[count..];
            Ok(())
        }
    }

    fn read_str_block(&mut self, block_size: usize) -> anyhow::Result<String> {
        if self.len() < block_size {
            Err(anyhow!("unexpected EOF"))
        } else {
            let mut str_ref = std::str::from_utf8(&self[..block_size])?;
            if let Some(idx) = str_ref.find('\0') {
                str_ref = &str_ref[..idx];
            }

            let result = str_ref.to_string();
            *self = &self[block_size..];
            Ok(result)
        }
    }

    fn read_str_nul(&mut self) -> anyhow::Result<String> {
        if let Some(idx) = self.iter().cloned().position(|b| b == 0) {
            let result = std::str::from_utf8(&self[..idx])?.to_string();
            *self = &self[idx + 1..];
            Ok(result)
        } else {
            Err(anyhow!("unexpected EOF"))
        }
    }

    fn read_utf16_nul(&mut self) -> anyhow::Result<String> {
        if let Some(idx) = self.windows(2).position(|window| window == [0, 0]) {
            let result = self[..idx]
                .chunks_exact(2)
                .map(|c| Endian::read_u16(c))
                .to_utf16chars()
                .map(|r| r.unwrap_or(Utf16Char::from('\u{fffd}')))
                .collect();
            *self = &self[idx + 2..];
            Ok(result)
        } else {
            Err(anyhow!("unexpected EOF"))
        }
    }

    fn read_utf16_pascal(&mut self) -> anyhow::Result<String> {
        let len = self.read_u16::<Endian>()? as usize;
        let bytes = &self[..len * 2];
        let result = utf16_slice_to_string(bytes);
        *self = &self[len * 2..];
        Ok(result)
    }

    fn read_utf16le_pascal(&mut self) -> anyhow::Result<String> {
        let len = self.read_u16::<Endian>()? as usize;
        let bytes = &self[..len * 2];
        let result = utf16le_slice_to_string(bytes);
        *self = &self[len * 2..];
        Ok(result)
    }

    fn read_entity_id(&mut self) -> anyhow::Result<EntityId> {
        Ok(EntityId::from_u32(self.read_u32::<Endian>()?))
    }

    fn read_direction(&mut self) -> anyhow::Result<Direction> {
        Ok(Direction::from_repr(self.read_u8()?).ok_or_else(|| anyhow!("invalid direction"))?)
    }
}

pub fn utf16_slice_to_string(bytes: &[u8]) -> String {
    bytes.chunks_exact(2)
        .map(|c| Endian::read_u16(c))
        .to_utf16chars()
        .map(|r| r.unwrap_or(Utf16Char::from('\u{fffd}')))
        .collect()
}

pub fn utf16le_slice_to_string(bytes: &[u8]) -> String {
    bytes.chunks_exact(2)
        .map(|c| LE::read_u16(c))
        .to_utf16chars()
        .map(|r| r.unwrap_or(Utf16Char::from('\u{fffd}')))
        .collect()
}


