use std::fmt;
use std::io::Write;
use std::ops::Deref;
use std::str::FromStr;

use anyhow::anyhow;
pub use byteorder::BigEndian as Endian;
use byteorder::ByteOrder;
use once_cell::sync::OnceCell;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

pub use client_version::{ClientFlags, ClientVersion, ExtendedClientVersion};
pub use format::{PacketReadExt, PacketWriteExt};
pub use login::*;

mod client_version;

mod login;

mod format;

pub trait Packet where Self: Sized {
    fn packet_kind() -> u8;
    fn fixed_length(client_version: ClientVersion) -> Option<usize>;

    fn decode(client_version: ClientVersion, payload: &[u8]) -> anyhow::Result<Self>;
    fn encode(&self, client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()>;
}

struct PacketRegistration {
    fixed_length: fn(client_version: ClientVersion) -> Option<usize>,
    decode: fn(client_version: ClientVersion, payload: &[u8]) -> anyhow::Result<Self>,
}

struct PacketRegistry {
    registrations: Vec<PacketRegistration>,
    max_size: usize,
}

static PACKET_REGISTRY: OnceCell<Vec<Option<PacketRegistration>>> = OnceCell::new();

fn get_packet_registry() -> &[Option<PacketRegistration>] {
    PACKET_REGISTRY.get_or_init(|| {
        let mut types = vec![None; 0x100];

        fn register_packet<T: Packet>() {
            types[T::packet_kind()] = Some(PacketRegistration {
                fixed_length: T::fixed_length,
                decode: T::decode,
            })
        }

        register_packet::<RegionLogin>();

        types
    })
}

macro_rules! all_packets {
    () => {{
        RegionLogin,
        SelectGameServer,
        GameServerLogin,
        CreateCharacterClassic,
        CreateCharacterEnhanced,
        DeleteCharacter,
        SelectCharacter,
    }}
}

macro_rules! parse_packet {
    ($self:expr, $kind:expr, $client_version:expr, $payload:expr, $($ty:ident),* $(,)*) => {{
        {
            let kind = $kind;
            $(if kind == <$ty>::packet_kind() {
                let length = if let Some(fixed_length) = <$ty>::fixed_length($client_version) {
                    fixed_length
                } else {
                    $self.reader.read_u16().await? as usize - 3
                };

                $payload.resize(length, 0u8);
                $self.reader.read_exact(&mut $payload[..]).await?;
                let decoded = <$ty>::decode($client_version, $payload)?;
                $payload.clear();
                Ok(AnyPacket::$ty(decoded))
            } else)*
            {
                return Err(anyhow!("Unknown packet type {}", kind));
            }
        }
    }}
}

macro_rules! send_packet {
    ($self:expr, $client_version:expr, $any:expr, $($ty:ident),* $(,)*) => {{
        match $any {
            AnyPacket::LegacySeed(seed) => $self.send_legacy_seed(seed).await,
            $(AnyPacket::$ty(v) => $self.send($client_version, &v).await,)*
        }
    }}
}

#[derive(Debug, Clone)]
pub enum AnyPacket {
    LegacySeed(u32),
    RegionLogin(RegionLogin),
    SelectGameServer(SelectGameServer),
    GameServerLogin(GameServerLogin),
    CreateCharacterClassic(CreateCharacterClassic),
    CreateCharacterEnhanced(CreateCharacterEnhanced),
    DeleteCharacter(DeleteCharacter),
    SelectCharacter(SelectCharacter),
}

pub struct Reader {
    reader: BufReader<OwnedReadHalf>,
    buffer: Vec<u8>,
    has_received: bool,
    client_version: ClientVersion,
}

impl Reader {
    pub async fn receive(&mut self, payload: &mut Vec<u8>) -> anyhow::Result<AnyPacket> {
        let packet_type = if self.has_received {
            self.has_received = false;

            // Legacy clients send their address immediately.
            // Newer clients send everything framed.
            // However, the packet ID of the new hello packet is 239, which is within the multicast
            // IP range, so it's safe to assume that seeing that byte means we're a new client.
            let first_byte = self.reader.read_u8().await?;
            if first_byte != 0xef {
                let mut addr_bytes = [first_byte, 0u8, 0u8, 0u8];
                self.reader.read_exact(&mut addr_bytes[1..]).await?;
                let addr = Endian::read_u32(&addr_bytes);
                return Ok(AnyPacket::LegacySeed(addr));
            }

            first_byte
        } else {
            self.reader.read_u8().await?
        };

        parse_packet!(self, packet_type, self.client_version, payload,
        )
    }
}

pub struct Writer {
    writer: BufWriter<OwnedWriteHalf>,
    buffer: Vec<u8>,
    has_sent: bool,
}

impl Writer {
    pub async fn send_legacy_seed(&mut self, seed: u32) -> anyhow::Result<()> {
        if self.has_sent {
            return Err(anyhow!("Tried to send legacy hello after other packets"));
        }
        self.has_sent = true;

        let mut addr_bytes = [0u8; 4];
        Endian::write_u32(&mut addr_bytes, seed);
        self.writer.write_all(&mut addr_bytes).await?;
        Ok(())
    }

    pub async fn send<T: Packet>(&mut self, client_version: ClientVersion, packet: &T)
        -> anyhow::Result<()> {
        self.has_sent = true;

        if let Some(length) = T::fixed_length(client_version) {
            self.buffer.reserve(length + 1);
            self.buffer.push(T::packet_kind());
            packet.encode(client_version, &mut self.buffer)?;
            assert_eq!(length, self.buffer.len(), "Fixed length packet wrote wrong size");
        } else {
            self.buffer.extend([T::packet_kind(), 0, 0]);
            packet.encode(client_version, &mut self.buffer)?;
            let packet_len = self.buffer.len() as u16;
            Endian::write_u16(&mut self.buffer[1..3], packet_len);
        }

        self.writer.write_all(&mut self.buffer).await?;
        self.buffer.clear();
        Ok(())
    }

    pub async fn send_any(&mut self, client_version: ClientVersion, packet: AnyPacket)
        -> anyhow::Result<()> {
        send_packet!(self, client_version, packet,
        )
    }
}

pub fn new_io(stream: TcpStream, is_server: bool) -> (Reader, Writer) {
    let (reader, writer) = stream.into_split();
    (Reader {
        reader: BufReader::new(reader),
        buffer: Vec::with_capacity(4096),
        has_received: is_server,
        client_version: ClientVersion::default(),
    }, Writer {
        writer: BufWriter::new(writer),
        buffer: Vec::with_capacity(4096),
        has_sent: is_server,
    })
}
