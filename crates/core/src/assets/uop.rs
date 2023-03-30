use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::num::Wrapping;
use std::ops::Deref;

use anyhow::anyhow;
use byteorder::{LittleEndian as Endian, ByteOrder, ReadBytesExt};
use flate2::read::ZlibDecoder;

const FILE_MAGIC: &[u8] = b"MYP\0";

fn partial_read_u32(s: &[u8]) -> Wrapping<u32> {
    let l = s.len();
    let mut v = 0;

    if l > 0 {
        v |= s[0] as u32;
    }

    if l > 1 {
        v |= (s[1] as u32) << 8;
    }

    if l > 2 {
        v |= (s[2] as u32) << 16;
    }

    if l > 3 {
        v |= (s[3] as u32) << 24;
    }

    Wrapping(v)
}

pub fn hash(mut src: &[u8]) -> u64 {
    let mut a = Wrapping((src.len() as u32).wrapping_add(0xdeadbeef));
    let mut b = a;
    let mut c = a;

    while src.len() > 12 {
        a += Wrapping(Endian::read_u32(src));
        b += Wrapping(Endian::read_u32(&src[4..]));
        c += Wrapping(Endian::read_u32(&src[8..]));

        a = (a - c) ^ ((c << 4) | (c >> 28));
        c += b;
        b = (b - a) ^ ((a << 6) | (a >> 26));
        a += c;
        c = (c - b) ^ ((b << 8) | (b >> 24));
        b += a;
        a = (a - c) ^ ((c << 16) | (c >> 16));
        c += b;
        b = (b - a) ^ ((a << 19) | (a >> 13));
        a += c;
        c = (c - b) ^ ((b << 4) | (b >> 28));
        b += a;

        src = &src[12..];
    }

    if src.len() > 0 {
        a += partial_read_u32(src);
        b += partial_read_u32(&src[4..]);
        c += partial_read_u32(&src[8..]);

        c = (c ^ b) - ((b << 14) | (b >> 18));
        a = (a ^ c) - ((c << 11) | (c >> 21));
        b = (b ^ a) - ((a << 25) | (a >> 7));
        c = (c ^ b) - ((b << 16) | (b >> 16));
        a = (a ^ c) - ((c << 4) | (c >> 28));
        b = (b ^ a) - ((a << 14) | (a >> 18));
        c = (c ^ b) - ((b << 24) | (b >> 8));
    }

    ((b.0 as u64) << 32) | (c.0 as u64)
}

#[derive(Debug, Clone)]
struct EntryInfo {
    offset: usize,
    header_length: usize,
    compressed_length: usize,
    decompressed_length: usize,
    is_compressed: bool,
}

pub struct UopBuffer<T> {
    backing: T,
    version: u32,
    format_timestamp: u32,
    block_size: u32,
    count: u32,
    entries: HashMap<u64, EntryInfo>,
}

impl<T: Deref<Target=[u8]>> UopBuffer<T> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn as_bytes(&self) -> &[u8] { self.backing.deref() }

    pub fn iter_hashes(&self) -> impl Iterator<Item = u64> + '_ { self.entries.keys().copied() }

    pub fn get(&self, key: &str) -> Option<Entry> {
        self.get_by_hash(hash(key.as_bytes()))
    }

    pub fn get_by_hash(&self, key_hash: u64) -> Option<Entry> {
        self.entries.get(&key_hash)
            .map(|v| {
                let (header, rest) = &self.backing[v.offset..].split_at(v.header_length);
                let bytes = &rest[..v.compressed_length];

                let stream = if v.is_compressed {
                    EntryStream::Zlib(ZlibDecoder::new(bytes))
                } else {
                    EntryStream::Uncompressed(bytes)
                };
                Entry {
                    info: v.clone(),
                    header,
                    stream,
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
        if version > 5 {
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
                let _value_crc = read.read_u32::<Endian>()?;
                let is_compressed = read.read_u16::<Endian>()?;
                entries.insert(key_hash, EntryInfo {
                    offset,
                    header_length,
                    compressed_length,
                    decompressed_length,
                    is_compressed: is_compressed == 1,
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

enum EntryStream<'a> {
    Uncompressed(&'a [u8]),
    Zlib(ZlibDecoder<&'a [u8]>),
}

pub struct Entry<'a> {
    info: EntryInfo,
    header: &'a [u8],
    stream: EntryStream<'a>,
}

impl<'a> Entry<'a> {
    pub fn header(&self) -> &[u8] { self.header }
    pub fn len(&self) -> usize { self.info.decompressed_length }
}

impl<'a> Read for Entry<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.stream {
            EntryStream::Uncompressed(s) => s.read(buf),
            EntryStream::Zlib(s) => s.read(buf),
        }
    }
}
