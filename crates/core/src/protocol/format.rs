use std::io::{Read, Write};
use byteorder::{WriteBytesExt, ReadBytesExt};
use super::{Endian};

static ZEROS: [u8; 1024] = [0u8; 1024];
static mut SCRATCH: [u8; 1024] = [0u8; 1024];

pub trait PacketWriteExt {
    fn write_zeros(&mut self, count: usize) -> anyhow::Result<()>;
    fn write_str_block(&mut self, src: &str, block_size: usize) -> anyhow::Result<()>;
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
}

pub trait PacketReadExt {
    fn skip(&mut self, count: usize) -> anyhow::Result<()>;
    fn read_str_block(&mut self, block_size: usize) -> anyhow::Result<String>;
}

impl<T: Read> PacketReadExt for T {
    fn skip(&mut self, mut count: usize) -> anyhow::Result<()> {
        while count > 0 {
            unsafe {
                let to_read = count.min(SCRATCH.len());
                self.read_exact(&mut SCRATCH[..to_read])?;
                count -= to_read;
            }
        }

        Ok(())
    }

    fn read_str_block(&mut self, block_size: usize) -> anyhow::Result<String> {
        let mut result = vec![0u8; block_size];
        self.read_exact(&mut result[..])?;
        let mut result = String::from_utf8(result)?;
        if let Some(idx) = result.find('\0') {
            result.truncate(idx);
        }
        Ok(result)
    }
}

