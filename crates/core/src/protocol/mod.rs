use std::any::type_name;
use std::fmt;
use std::fmt::Debug;
use std::io::Write;
use std::mem::{MaybeUninit, size_of, transmute};

use anyhow::anyhow;
use byteorder::{ByteOrder, WriteBytesExt};
pub use byteorder::BigEndian as Endian;
use once_cell::sync::OnceCell;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

pub use client_version::{ClientFlags, ClientVersion, ExtendedClientVersion};
pub use format::{PacketReadExt, PacketWriteExt};
pub use login::*;
pub use map::*;
pub use extended::*;
pub use input::*;
pub use entity::*;

use crate::protocol::compression::{HuffmanVecWriter};
use crate::protocol::sound::PlayMusic;
use crate::protocol::ui::OpenChatWindow;

mod format;

pub mod compression;

mod client_version;

mod login;

mod map;

mod extended;

mod input;

mod entity;

mod ui;

mod sound;

pub trait Packet where Self: Sized {
    fn packet_kind() -> u8;
    fn fixed_length(client_version: ClientVersion) -> Option<usize>;

    fn decode(client_version: ClientVersion, payload: &[u8]) -> anyhow::Result<Self>;
    fn encode(&self, client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()>;
}

#[derive(Clone)]
struct PacketRegistration {
    packet_kind: u8,
    size: usize,
    drop: fn(*mut ()),
    fixed_length: fn(client_version: ClientVersion) -> Option<usize>,
    decode: fn(client_version: ClientVersion, payload: &[u8]) -> anyhow::Result<AnyPacket>,
    encode: fn(client_version: ClientVersion, writer: &mut dyn Write, ptr: *mut ()) -> anyhow::Result<()>,
    debug: fn(ptr: *mut (), f: &mut fmt::Formatter<'_>) -> fmt::Result,
}

impl PacketRegistration {
    pub fn for_type<T: Packet + Debug>() -> PacketRegistration {
        fn drop_packet<T: Packet>(ptr: *mut ()) {
            unsafe { std::ptr::drop_in_place(ptr as *mut T) }
        }

        fn decode_packet<T: Packet>(client_version: ClientVersion, payload: &[u8]) -> anyhow::Result<AnyPacket> {
            log::debug!("Decoding {}", type_name::<T>());
            Ok(AnyPacket::from_packet(T::decode(client_version, payload)?))
        }

        fn encode_packet<T: Packet>(client_version: ClientVersion,
            mut writer: &mut dyn Write, ptr: *mut ()) -> anyhow::Result<()> {
            let packet = unsafe { &*(ptr as *const T) };
            packet.encode(client_version, &mut writer)
        }

        fn debug<T: Debug>(ptr: *mut (), f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let packet = unsafe { &*(ptr as *const T) };
            packet.fmt(f)
        }

        PacketRegistration {
            packet_kind: T::packet_kind(),
            size: size_of::<T>(),
            drop: drop_packet::<T>,
            fixed_length: T::fixed_length,
            decode: decode_packet::<T>,
            encode: encode_packet::<T>,
            debug: debug::<T>,
        }
    }
}

struct PacketRegistry {
    registrations: Vec<Option<PacketRegistration>>,
}

const MAX_PACKET_STRUCT_SIZE: usize = 128;
static PACKET_REGISTRY: OnceCell<PacketRegistry> = OnceCell::new();

fn packet_registry() -> &'static PacketRegistry {
    PACKET_REGISTRY.get_or_init(|| {
        let mut registrations = vec![None; 0x100];
        let mut max_size = 0usize;

        for registration in [
            // Add packet types here. It's not ideal but it works for now.

            // Login
            PacketRegistration::for_type::<Seed>(),
            PacketRegistration::for_type::<AccountLogin>(),
            PacketRegistration::for_type::<ServerList>(),
            PacketRegistration::for_type::<SelectGameServer>(),
            PacketRegistration::for_type::<SwitchServer>(),
            PacketRegistration::for_type::<GameServerLogin>(),
            PacketRegistration::for_type::<SupportedFeatures>(),
            PacketRegistration::for_type::<CharacterList>(),
            PacketRegistration::for_type::<CreateCharacterClassic>(),
            PacketRegistration::for_type::<CreateCharacterEnhanced>(),
            PacketRegistration::for_type::<DeleteCharacter>(),
            PacketRegistration::for_type::<SelectCharacter>(),
            PacketRegistration::for_type::<ClientVersionRequest>(),
            PacketRegistration::for_type::<BeginEnterWorld>(),
            PacketRegistration::for_type::<EndEnterWorld>(),
            PacketRegistration::for_type::<ShowPublicHouses>(),
            PacketRegistration::for_type::<Ping>(),
            PacketRegistration::for_type::<Logout>(),
            PacketRegistration::for_type::<WarMode>(),

            // Extended
            PacketRegistration::for_type::<ExtendedCommand>(),

            // Input
            PacketRegistration::for_type::<Move>(),
            PacketRegistration::for_type::<MoveConfirm>(),
            PacketRegistration::for_type::<MoveReject>(),
            PacketRegistration::for_type::<SingleClick>(),
            PacketRegistration::for_type::<DoubleClick>(),

            // UI
            PacketRegistration::for_type::<OpenChatWindow>(),

            // Sound
            PacketRegistration::for_type::<PlayMusic>(),

            // Map
            PacketRegistration::for_type::<SetTime>(),
            PacketRegistration::for_type::<ChangeSeason>(),
            PacketRegistration::for_type::<ViewRange>(),

            // Entity
            PacketRegistration::for_type::<EntityRequest>(),
            PacketRegistration::for_type::<UpsertEntityLegacy>(),
            PacketRegistration::for_type::<UpsertEntity>(),
            PacketRegistration::for_type::<DeleteEntity>(),
            PacketRegistration::for_type::<UpsertLocalPlayer>(),
            PacketRegistration::for_type::<UpsertEntityCharacter>(),
            PacketRegistration::for_type::<UpsertEntityStats>(),
        ].into_iter() {
            max_size = registration.size.max(max_size);
            let index = registration.packet_kind as usize;
            registrations[index] = Some(registration);
        }

        assert_eq!(max_size, MAX_PACKET_STRUCT_SIZE, "MAX_PACKET_STRUCT_SIZE is out of date. Should be {max_size}.");
        PacketRegistry {
            registrations,
        }
    })
}

pub struct AnyPacket {
    kind: u8,
    _pad: [u8; 3],
    buffer: [u8; MAX_PACKET_STRUCT_SIZE],
}

impl AnyPacket {
    fn registration(&self) -> &PacketRegistration {
        packet_registry().registrations[self.kind as usize].as_ref().unwrap()
    }

    pub fn packet_kind(&self) -> u8 { self.kind }

    pub fn fixed_length(&self, client_version: ClientVersion) -> Option<usize> {
        (self.registration().fixed_length)(client_version)
    }

    pub fn from_packet<P: Packet>(packet: P) -> AnyPacket {
        assert!(size_of::<P>() <= MAX_PACKET_STRUCT_SIZE, "packet is too large");

        unsafe {
            let mut new_packet = MaybeUninit::<AnyPacket>::uninit();
            let ptr = new_packet.as_mut_ptr();
            (*ptr).kind = P::packet_kind();
            std::ptr::write(transmute(&mut (*ptr).buffer), packet);
            new_packet.assume_init()
        }
    }

    pub fn downcast<P: Packet>(&self) -> Option<&P> {
        if P::packet_kind() == self.kind {
            Some(unsafe { transmute(&self.buffer) })
        } else {
            None
        }
    }

    pub fn downcast_mut<P: Packet>(&mut self) -> Option<&mut P> {
        if P::packet_kind() == self.kind {
            Some(unsafe { transmute(&mut self.buffer) })
        } else {
            None
        }
    }

    pub fn into_downcast<P: Packet>(self) -> Result<P, Self> {
        if P::packet_kind() == self.kind {
            let result = Ok(unsafe { std::ptr::read(transmute(&self.buffer)) });
            std::mem::forget(self);
            result
        } else {
            Err(self)
        }
    }

    pub fn encode(&self, client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        (self.registration().encode)(client_version, writer, unsafe { transmute(&self.buffer) })
    }
}

impl Debug for AnyPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            (self.registration().debug)(transmute(&self.buffer), f)
        }
    }
}

impl Drop for AnyPacket {
    fn drop(&mut self) {
        let registration = self.registration();

        unsafe {
            (registration.drop)(transmute(&self.buffer))
        }
    }
}

impl<T: Packet> From<T> for AnyPacket {
    fn from(packet: T) -> Self {
        AnyPacket::from_packet(packet)
    }
}

pub struct Reader {
    reader: BufReader<OwnedReadHalf>,
    buffer: Vec<u8>,
}

impl Reader {
    pub async fn recv(&mut self, client_version: ClientVersion)
        -> anyhow::Result<AnyPacket> {
        let packet_kind = self.reader.read_u8().await?;
        let registry = packet_registry();
        let registration = match registry.registrations[packet_kind as usize].as_ref() {
            Some(r) => r,
            None => {
                return Err(anyhow!("Unknown packet type {packet_kind:2x}"));
            }
        };

        let length = if let Some(fixed_length) = (registration.fixed_length)(client_version) {
            fixed_length - 1
        } else {
            self.reader.read_u16().await? as usize - 3
        };

        log::debug!("Beginning {packet_kind:2x} length {length}");

        self.buffer.resize(length, 0u8);
        self.reader.read_exact(&mut self.buffer[..]).await?;

        let decoded = (registration.decode)(client_version, &self.buffer)?;
        self.buffer.clear();
        Ok(decoded)
    }
}

pub struct Writer {
    writer: BufWriter<OwnedWriteHalf>,
    buffer: Vec<u8>,
    has_sent: bool,
    compress: bool,
    compress_buffer: Vec<u8>,
}

impl Writer {
    pub fn enable_compression(&mut self) {
        self.compress = true;
    }

    pub async fn send_legacy_seed(&mut self, seed: u32) -> anyhow::Result<()> {
        if self.has_sent {
            return Err(anyhow!("Tried to send legacy hello after other packets"));
        }
        self.has_sent = true;

        let mut addr_bytes = [0u8; 4];
        Endian::write_u32(&mut addr_bytes, seed);
        self.writer.write_all(&mut addr_bytes).await?;
        self.writer.flush().await?;
        Ok(())
    }

    pub async fn send<T: Packet>(&mut self, client_version: ClientVersion, packet: &T)
        -> anyhow::Result<()> {
        self.has_sent = true;

        match (T::fixed_length(client_version), self.compress) {
            (Some(_), true) => {
                let mut writer = HuffmanVecWriter::new(&mut self.buffer);
                writer.write_u8(T::packet_kind())?;
                packet.encode(client_version, &mut writer)?;
                writer.finish();
            }
            (Some(length), false) => {
                self.buffer.reserve(length);
                self.buffer.push(T::packet_kind());
                packet.encode(client_version, &mut self.buffer)?;
                assert_eq!(length, self.buffer.len(), "Fixed length packet wrote wrong size");
            }
            (None, true) => {
                self.compress_buffer.clear();
                self.compress_buffer.reserve(4096);
                self.compress_buffer.extend([T::packet_kind(), 0, 0]);
                packet.encode(client_version, &mut self.compress_buffer)?;
                let packet_len = self.compress_buffer.len() as u16;
                Endian::write_u16(&mut self.compress_buffer[1..3], packet_len);
                let mut writer = HuffmanVecWriter::new(&mut self.buffer);
                writer.write(&self.compress_buffer)?;
                writer.finish();
                self.compress_buffer.clear();
            }
            (None, false) => {
                self.buffer.extend([T::packet_kind(), 0, 0]);
                packet.encode(client_version, &mut self.buffer)?;
                let packet_len = self.buffer.len() as u16;
                Endian::write_u16(&mut self.buffer[1..3], packet_len);
            }
        }

        self.writer.write_all(&mut self.buffer).await?;
        self.buffer.clear();
        self.writer.flush().await?;
        Ok(())
    }

    pub async fn send_any(&mut self, client_version: ClientVersion, packet: &AnyPacket)
        -> anyhow::Result<()> {
        self.has_sent = true;

        let kind = packet.packet_kind();
        match (packet.fixed_length(client_version), self.compress) {
            (Some(_), true) => {
                let mut writer = HuffmanVecWriter::new(&mut self.buffer);
                writer.write_u8(kind)?;
                packet.encode(client_version, &mut writer)?;
                writer.finish();
            }
            (Some(length), false) => {
                self.buffer.reserve(length);
                self.buffer.push(kind);
                packet.encode(client_version, &mut self.buffer)?;
                assert_eq!(length, self.buffer.len(), "Fixed length packet wrote wrong size");
            }
            (None, true) => {
                self.compress_buffer.clear();
                self.compress_buffer.reserve(4096);
                self.compress_buffer.extend([kind, 0, 0]);
                packet.encode(client_version, &mut self.compress_buffer)?;
                let packet_len = self.compress_buffer.len() as u16;
                Endian::write_u16(&mut self.compress_buffer[1..3], packet_len);
                let mut writer = HuffmanVecWriter::new(&mut self.buffer);
                writer.write(&self.compress_buffer)?;
                writer.finish();
                self.compress_buffer.clear();
            }
            (None, false) => {
                self.buffer.extend([kind, 0, 0]);
                packet.encode(client_version, &mut self.buffer)?;
                let packet_len = self.buffer.len() as u16;
                Endian::write_u16(&mut self.buffer[1..3], packet_len);
            }
        }

        self.writer.write_all(&mut self.buffer).await?;
        self.buffer.clear();
        self.writer.flush().await?;
        Ok(())
    }
}

pub fn new_io(stream: TcpStream, is_server: bool) -> (Reader, Writer) {
    let (reader, writer) = stream.into_split();
    (Reader {
        reader: BufReader::new(reader),
        buffer: Vec::with_capacity(4096),
    }, Writer {
        writer: BufWriter::new(writer),
        buffer: Vec::with_capacity(4096),
        has_sent: is_server,
        compress: false,
        compress_buffer: Vec::new(),
    })
}
