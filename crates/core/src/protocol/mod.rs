use std::any::type_name;
use std::io::{ErrorKind, Write};

use anyhow::{anyhow, bail};
pub use byteorder::BigEndian as Endian;
use byteorder::ByteOrder;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tracing::{trace, warn};

use compression::HuffmanVecWriter;
use encryption::Encryption;

pub use character::*;
pub use chat::*;
pub use client_version::{ClientFlags, ClientVersion, ExtendedClientVersion};
pub use entity::*;
pub use extended::*;
pub use format::{PacketReadExt, PacketWriteExt};
pub use input::*;
pub use login::*;
pub use map::*;
pub use sound::*;
pub use ui::*;
pub use any::{AnyPacket, IntoAnyPacket};

mod format;

pub mod compression;

pub mod encryption;

mod client_version;

mod login;

mod map;

mod extended;

mod input;

mod entity;

mod ui;

mod sound;

mod chat;

mod character;

mod any;

pub trait Packet
where
    Self: Sized,
{
    const PACKET_KIND: u8;

    fn fixed_length(client_version: ClientVersion) -> Option<usize>;

    fn decode(client_version: ClientVersion, from_client: bool, payload: &[u8]) -> anyhow::Result<Self>;
    fn encode(&self, client_version: ClientVersion, to_client: bool, writer: &mut impl Write) -> anyhow::Result<()>;
}

pub trait OutgoingPacket {
    fn packet_type_name(&self) -> &'static str;
    fn packet_kind(&self) -> u8;
    fn fixed_length(&self, client_version: ClientVersion) -> Option<usize>;
    fn encode(&self, client_version: ClientVersion, to_client: bool, writer: &mut impl Write) -> anyhow::Result<()>;
}

impl<T: Packet> OutgoingPacket for T {
    fn packet_type_name(&self) -> &'static str {
        type_name::<T>()
    }

    fn packet_kind(&self) -> u8 {
        T::PACKET_KIND
    }

    fn fixed_length(&self, client_version: ClientVersion) -> Option<usize> {
        T::fixed_length(client_version)
    }

    fn encode(
        &self, client_version: ClientVersion, to_client: bool, writer: &mut impl Write,
    ) -> anyhow::Result<()> {
        <T as Packet>::encode(self, client_version, to_client, writer)
    }
}

pub struct Reader {
    reader: OwnedReadHalf,
    encryption: Option<Encryption>,
    buffer: Vec<u8>,
    buffer_offset: usize,
    buffer_len: usize,
    from_client: bool,
}

impl Reader {
    fn new(reader: OwnedReadHalf, from_client: bool) -> Reader {
        Reader {
            reader,
            encryption: None,
            buffer: Vec::with_capacity(4096),
            buffer_offset: 0,
            buffer_len: 0,
            from_client,
        }
    }

    pub fn set_encryption(&mut self, mut encryption: Option<Encryption>) {
        if self.encryption.is_some() {
            warn!("Tried to disable encryption. This could cause issues");
        }

        if let Some(encryption) = &mut encryption {
            let offset = self.buffer_offset;
            let len = self.buffer_len;
            let queued_slice = &mut self.buffer[offset..(offset + len)];
            Self::encrypt(Some(encryption), self.from_client, queued_slice);
        }

        self.encryption = encryption;
    }

    fn encrypt(encryption: Option<&mut Encryption>, from_client: bool, buffer: &mut [u8]) {
        if let Some(encryption) = encryption {
            if from_client {
                encryption.crypt_client_to_server(buffer);
            } else {
                encryption.crypt_server_to_client(buffer);
            }
        }
    }

    async fn read(&mut self, n: usize) -> std::io::Result<&[u8]> {
        while self.buffer_len < n {
            let offset = self.buffer_offset;
            let required_vec_size = offset + n;
            if self.buffer.len() < required_vec_size {
                self.buffer.resize(required_vec_size, 0);
            }

            let read_offset = self.buffer_offset + self.buffer_len;
            let n = self.reader.read(&mut self.buffer[read_offset..]).await?;
            if n == 0 {
                return Err(std::io::Error::new(ErrorKind::UnexpectedEof, "eof"));
            }

            self.buffer_len += n;
            let read_bytes = &mut self.buffer[read_offset..(read_offset + n)];
            Self::encrypt(self.encryption.as_mut(), self.from_client, read_bytes);
        }

        Ok(&self.buffer[self.buffer_offset..(self.buffer_offset + n)])
    }

    fn consume(&mut self, n: usize) {
        assert!(self.buffer_len >= n);
        self.buffer_offset += n;
        self.buffer_len -= n;

        if self.buffer_len == 0 {
            self.buffer_offset = 0;
        }
    }

    pub async fn recv(
        &mut self, client_version: ClientVersion,
    ) -> anyhow::Result<Option<AnyPacket>> {
        let packet_kind_slice = match self.read(1).await {
            Ok(v) => v,
            Err(err) if err.kind() == ErrorKind::UnexpectedEof => return Ok(None),
            Err(err) => return Err(err.into()),
        };

        let packet_kind = packet_kind_slice[0];
        self.consume(1);
        let registration = AnyPacket::registration_for(packet_kind)
            .ok_or_else(|| anyhow!("Unknown packet type {packet_kind:2x}"))?;

        let length = if let Some(fixed_length) = (registration.fixed_length)(client_version) {
            fixed_length - 1
        } else {
            let bytes = self.read(2).await?;
            let length = Endian::read_u16(bytes) as usize;
            if length < 3 {
                bail!("invalid packet length {length}");
            }

            self.consume(2);
            length - 3
        };

        trace!("RECV: {packet_kind:2x} {} length={length}", registration.type_name);

        let from_client = self.from_client;
        let buffer = self.read(length).await?;
        let decoded = (registration.decode)(client_version, from_client, buffer);
        self.consume(length);
        Ok(Some(decoded?))
    }
}

pub struct Writer {
    writer: BufWriter<OwnedWriteHalf>,
    buffer: Vec<u8>,
    has_sent: bool,
    to_client: bool,
    compress: bool,
    compress_buffer: Vec<u8>,
    encryption: Option<Encryption>,
}

impl Writer {
    fn new(writer: OwnedWriteHalf, to_client: bool) -> Writer {
        Writer {
            writer: BufWriter::new(writer),
            buffer: Vec::with_capacity(4096),
            has_sent: to_client,
            to_client,
            compress: false,
            compress_buffer: Vec::new(),
            encryption: None,
        }
    }

    pub fn enable_compression(&mut self) {
        self.compress = true;
    }

    pub fn set_encryption(&mut self, encryption: Option<Encryption>) {
        self.encryption = encryption;
    }

    pub async fn send_legacy_seed(&mut self, seed: u32) -> anyhow::Result<()> {
        if self.has_sent {
            return Err(anyhow!("Tried to send legacy hello after other packets"));
        }
        self.has_sent = true;

        let mut addr_bytes = [0u8; 4];
        Endian::write_u32(&mut addr_bytes, seed);
        self.writer.write_all(&addr_bytes).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn send_raw(&mut self) -> anyhow::Result<()> {
        self.has_sent = true;

        if self.compress {
            std::mem::swap(&mut self.buffer, &mut self.compress_buffer);
            let mut writer = HuffmanVecWriter::new(&mut self.buffer);
            writer.write_all(&self.compress_buffer)?;
            writer.finish();
            self.compress_buffer.clear();
        }

        if let Some(encryption) = self.encryption.as_mut() {
            if self.to_client {
                encryption.crypt_server_to_client(&mut self.buffer);
            } else {
                encryption.crypt_client_to_server(&mut self.buffer);
            }
        }

        let result = self.writer.write_all(&self.buffer).await;
        self.buffer.clear();
        result?;
        self.writer.flush().await?;
        Ok(())
    }

    pub async fn send(
        &mut self, client_version: ClientVersion, packet: &impl OutgoingPacket,
    ) -> anyhow::Result<()> {
        let packet_kind = packet.packet_kind();
        let type_name = packet.packet_type_name();
        trace!("SEND: {:2x} {}", packet_kind, type_name);

        if let Some(length) = packet.fixed_length(client_version) {
            self.buffer.reserve(length);
            self.buffer.push(packet_kind);
            packet.encode(client_version, self.to_client, &mut self.buffer)?;
            assert_eq!(length, self.buffer.len(), "Fixed length packet wrote wrong size");
        } else {
            self.buffer.extend([packet_kind, 0, 0]);
            packet.encode(client_version, self.to_client, &mut self.buffer)?;
            let packet_len = self.buffer.len() as u16;
            Endian::write_u16(&mut self.buffer[1..3], packet_len);
        }

        self.send_raw().await
    }
}

pub fn new_io(stream: TcpStream, is_server: bool) -> (Reader, Writer) {
    let (reader, writer) = stream.into_split();
    (Reader::new(reader, is_server), Writer::new(writer, is_server))
}
