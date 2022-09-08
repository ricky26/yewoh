use std::io::{Write};
use anyhow::anyhow;
use byteorder::{WriteBytesExt};

static ZEROS: [u8; 1024] = [0u8; 1024];

pub trait PacketWriteExt {
    fn write_zeros(&mut self, count: usize) -> anyhow::Result<()>;
    fn write_str_block(&mut self, src: &str, block_size: usize) -> anyhow::Result<()>;
    fn write_str_nul(&mut self, src: &str) -> anyhow::Result<()>;
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
}

pub trait PacketReadExt {
    fn skip(&mut self, count: usize) -> anyhow::Result<()>;
    fn read_str_block(&mut self, block_size: usize) -> anyhow::Result<String>;
    fn read_str_nul(&mut self) -> anyhow::Result<String>;
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
}

