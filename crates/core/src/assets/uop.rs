use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::ops::Deref;

use anyhow::anyhow;
use byteorder::{LittleEndian as Endian, ReadBytesExt};
use flate2::read::ZlibDecoder;

const FILE_MAGIC: &[u8] = b"MYP\0";

pub fn hash(mut src: &[u8]) -> u64 {
    let mut eax = 0u32;
    let mut ebx = src.len() as u32 + 0xdeadbeefu32;
    let mut edi = ebx;
    let mut esi = ebx;

    fn get(src: &[u8], offset: usize) -> u32 { src[offset] as u32 }

    while src.len() >= 12 {
        edi = ((get(src, 7) << 24) | (get(src, 6) << 16) | (get(src, 5) << 8) | get(src, 4)) + edi;
        esi = ((get(src, 11) << 24) | (get(src, 10) << 16) | (get(src, 9) << 8) | get(src, 8)) + esi;
        let mut edx = ((get(src, 3) << 24) | (get(src, 2) << 16) | (get(src, 1) << 8) | get(src, 0)) - esi;
        edx = (edx + ebx) ^ (esi >> 28) ^ (esi << 4);
        esi += edi;
        edi = (edi - edx) ^ (edx >> 26) ^ (edx << 6);
        edx += esi;
        esi = (esi - edi) ^ (edi >> 24) ^ (edi << 8);
        edi += edx;
        ebx = (edx - esi) ^ (esi >> 16) ^ (esi << 16);
        esi += edi;
        edi = (edi - ebx) ^ (ebx >> 13) ^ (ebx << 19);
        ebx += esi;
        esi = (esi - edi) ^ (edi >> 28) ^ (edi << 4);
        edi += ebx;
        src = &src[12..];
    }

    for i in (1..(src.len())).rev() {
        match i {
            11 => esi += get(src, 10) << 16,
            10 => esi += get(src, 9) << 8,
            9 => esi += get(src, 8),
            8 => edi += get(src, 7) << 24,
            7 => edi += get(src, 6) << 16,
            6 => edi += get(src, 5) << 8,
            5 => edi += get(src, 4),
            4 => ebx += get(src, 3) << 24,
            3 => ebx += get(src, 2) << 16,
            2 => ebx += get(src, 1) << 8,
            1 => {
                ebx += get(src, 0);
                esi = (esi ^ edi) - ((edi >> 18) ^ (edi << 14));
                let ecx = (esi ^ ebx) - ((esi >> 21) ^ (esi << 11));
                edi = (edi ^ ecx) - ((ecx >> 7) ^ (ecx << 25));
                esi = (esi ^ edi) - ((edi >> 16) ^ (edi << 16));
                let edx = (esi ^ ecx) - ((esi >> 28) ^ (esi << 4));
                edi = (edi ^ edx) - ((edx >> 18) ^ (edx << 14));
                eax = (esi ^ edi) - ((edi >> 8) ^ (edi << 24));
                esi = edi;
            }
            _ => unreachable!(),
        }
    }

    ((esi as u64) << 32) | (eax as u64)
}

#[derive(Debug, Clone)]
struct Entry {
    offset: usize,
    header_length: usize,
    compressed_length: usize,
    decompressed_length: usize,
    is_compressed: bool,
    crc: u32,
}

pub struct UopBuffer<T> {
    backing: T,
    version: u32,
    format_timestamp: u32,
    block_size: u32,
    count: u32,
    entries: HashMap<u64, Entry>,
}

impl<T: Deref<Target=[u8]>> UopBuffer<T> {
    pub fn as_bytes(&self) -> &[u8] { self.backing.deref() }

    pub fn get(&self, key: &str) -> Option<EntryStream> {
        self.get_by_hash(hash(key.as_bytes()))
    }

    pub fn get_by_hash(&self, key_hash: u64) -> Option<EntryStream> {
        self.entries.get(&key_hash)
            .map(|v| {
                let bytes = &self.backing[v.offset..(v.offset + v.compressed_length)];
                if v.is_compressed {
                    EntryStream::Zlib(ZlibDecoder::new(bytes))
                } else {
                    EntryStream::Uncompressed(bytes)
                }
            })
    }

    pub fn try_from_backing(backing: T) -> anyhow::Result<UopBuffer<T>> {
        let bytes = backing.deref();
        if !bytes.starts_with(FILE_MAGIC) {
            return Err(anyhow!("Invalid UOP file header"));
        }

        let mut read = &bytes[FILE_MAGIC.len()..];
        let version = read.read_u32::<Endian>()?;
        if version != 5 {
            return Err(anyhow!("Unsupported UOP version {version}"));
        }
        let format_timestamp = read.read_u32::<Endian>()?;
        let mut next_block_offset = read.read_u64::<Endian>()?;
        let block_size = read.read_u32::<Endian>()?;
        let count = read.read_u32::<Endian>()?;
        let mut entries = HashMap::new();

        while next_block_offset != 0 {
            let mut read = &bytes[next_block_offset as usize..];
            let file_count = read.read_u32::<Endian>()?;
            next_block_offset = read.read_u64::<Endian>()?;

            for _ in 0..file_count {
                let offset = read.read_u64::<Endian>()? as usize;
                let header_length = read.read_u32::<Endian>()? as usize;
                let compressed_length = read.read_u32::<Endian>()? as usize;
                let decompressed_length = read.read_u32::<Endian>()? as usize;
                let key_hash = read.read_u64::<Endian>()?;
                let value_crc = read.read_u32::<Endian>()?;
                let is_compressed = read.read_u16::<Endian>()?;
                entries.insert(key_hash, Entry {
                    offset,
                    header_length,
                    compressed_length,
                    decompressed_length,
                    is_compressed: is_compressed == 1,
                    crc: value_crc,
                });
            }
        }

        Ok(UopBuffer {
            backing,
            version,
            format_timestamp,
            block_size,
            count,
            entries,
        })
    }
}

impl<T: Clone> Clone for UopBuffer<T> {
    fn clone(&self) -> Self {
        Self {
            backing: self.backing.clone(),
            version: self.version,
            format_timestamp: self.format_timestamp,
            block_size: self.block_size,
            count: self.count,
            entries: self.entries.clone(),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for UopBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UopBuffer")
            .field("backing", &self.backing)
            .field("version", &self.version)
            .field("format_timestamp", &self.format_timestamp)
            .field("block_size", &self.block_size)
            .field("count", &self.count)
            .field("entries", &self.entries)
            .finish()
    }
}

pub enum EntryStream<'a> {
    Uncompressed(&'a [u8]),
    Zlib(ZlibDecoder<&'a [u8]>),
}

impl<'a> Read for EntryStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            EntryStream::Uncompressed(s) => s.read(buf),
            EntryStream::Zlib(s) => s.read(buf),
        }
    }
}
