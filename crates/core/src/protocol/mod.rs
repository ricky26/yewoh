use std::any::type_name;
use std::fmt;
use std::fmt::Debug;
use std::io::Write;
use std::mem::{MaybeUninit, size_of, transmute};
use std::sync::Arc;

use anyhow::anyhow;
use byteorder::ByteOrder;
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
pub use chat::*;
pub use sound::*;
pub use ui::*;
pub use character::*;

use crate::protocol::compression::{HuffmanVecWriter};
use crate::protocol::encryption::Encryption;

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

pub trait Packet where Self: Sized {
    fn packet_kind() -> u8;
    fn fixed_length(client_version: ClientVersion) -> Option<usize>;

    fn decode(client_version: ClientVersion, from_client: bool, payload: &[u8]) -> anyhow::Result<Self>;
    fn encode(&self, client_version: ClientVersion, to_client: bool, writer: &mut impl Write) -> anyhow::Result<()>;

    fn into_arc(self) -> Arc<AnyPacket> { Arc::from(AnyPacket::from(self)) }
}

#[derive(Clone)]
struct PacketRegistration {
    packet_kind: u8,
    size: usize,
    drop: fn(*mut ()),
    fixed_length: fn(client_version: ClientVersion) -> Option<usize>,
    decode: fn(client_version: ClientVersion, from_client: bool, payload: &[u8]) -> anyhow::Result<AnyPacket>,
    encode: fn(client_version: ClientVersion, to_client: bool, writer: &mut dyn Write, ptr: *mut ()) -> anyhow::Result<()>,
    clone: fn(ptr: *mut ()) -> AnyPacket,
    debug: fn(ptr: *mut (), f: &mut fmt::Formatter<'_>) -> fmt::Result,
}

impl PacketRegistration {
    pub fn for_type<T: Packet + Debug + Clone>() -> PacketRegistration {
        fn drop_packet<T: Packet>(ptr: *mut ()) {
            unsafe { std::ptr::drop_in_place(ptr as *mut T) }
        }

        fn decode_packet<T: Packet>(client_version: ClientVersion, from_client: bool,
            payload: &[u8]) -> anyhow::Result<AnyPacket> {
            log::trace!("Decoding {}", type_name::<T>());
            Ok(AnyPacket::from_packet(T::decode(client_version, from_client, payload)?))
        }

        fn encode_packet<T: Packet>(client_version: ClientVersion, to_client: bool,
            mut writer: &mut dyn Write, ptr: *mut ()) -> anyhow::Result<()> {
            let packet = unsafe { &*(ptr as *const T) };
            packet.encode(client_version, to_client, &mut writer)
        }

        fn debug<T: Debug>(ptr: *mut (), f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let packet = unsafe { &*(ptr as *const T) };
            packet.fmt(f)
        }

        fn clone<T: Packet + Clone>(ptr: *mut ()) -> AnyPacket {
            let packet = unsafe { &*(ptr as *const T) };
            AnyPacket::from_packet(packet.clone())
        }

        PacketRegistration {
            packet_kind: T::packet_kind(),
            size: size_of::<T>(),
            drop: drop_packet::<T>,
            fixed_length: T::fixed_length,
            decode: decode_packet::<T>,
            encode: encode_packet::<T>,
            debug: debug::<T>,
            clone: clone::<T>,
        }
    }
}

struct PacketRegistry {
    registrations: Vec<Option<PacketRegistration>>,
}

const MAX_PACKET_STRUCT_SIZE: usize = 136;
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
            PacketRegistration::for_type::<RequestHelp>(),

            // Extended
            PacketRegistration::for_type::<ExtendedCommand>(),
            PacketRegistration::for_type::<ExtendedCommandAos>(),

            // Input
            PacketRegistration::for_type::<Move>(),
            PacketRegistration::for_type::<MoveConfirm>(),
            PacketRegistration::for_type::<MoveReject>(),
            PacketRegistration::for_type::<SingleClick>(),
            PacketRegistration::for_type::<DoubleClick>(),
            PacketRegistration::for_type::<PickUpEntity>(),
            PacketRegistration::for_type::<DropEntity>(),
            PacketRegistration::for_type::<MoveEntityReject>(),
            PacketRegistration::for_type::<EquipEntity>(),
            PacketRegistration::for_type::<PickTarget>(),

            // UI
            PacketRegistration::for_type::<OpenChatWindow>(),
            PacketRegistration::for_type::<OpenPaperDoll>(),
            PacketRegistration::for_type::<OpenContainer>(),
            PacketRegistration::for_type::<OpenGump>(),
            PacketRegistration::for_type::<OpenGumpCompressed>(),
            PacketRegistration::for_type::<GumpResult>(),

            // Chat
            PacketRegistration::for_type::<AsciiTextMessage>(),
            PacketRegistration::for_type::<UnicodeTextMessage>(),
            PacketRegistration::for_type::<LocalisedTextMessage>(),
            PacketRegistration::for_type::<AsciiTextMessageRequest>(),
            PacketRegistration::for_type::<UnicodeTextMessageRequest>(),

            // Sound
            PacketRegistration::for_type::<PlayMusic>(),
            PacketRegistration::for_type::<PlaySoundEffect>(),

            // Map
            PacketRegistration::for_type::<SetTime>(),
            PacketRegistration::for_type::<ChangeSeason>(),
            PacketRegistration::for_type::<ViewRange>(),
            PacketRegistration::for_type::<GlobalLightLevel>(),

            // Entity
            PacketRegistration::for_type::<EntityRequest>(),
            PacketRegistration::for_type::<UpsertEntityLegacy>(),
            PacketRegistration::for_type::<UpsertEntityWorld>(),
            PacketRegistration::for_type::<DeleteEntity>(),
            PacketRegistration::for_type::<UpsertLocalPlayer>(),
            PacketRegistration::for_type::<UpsertEntityCharacter>(),
            PacketRegistration::for_type::<UpdateCharacter>(),
            PacketRegistration::for_type::<UpsertEntityEquipped>(),
            PacketRegistration::for_type::<UpsertEntityContained>(),
            PacketRegistration::for_type::<UpsertContainerContents>(),
            PacketRegistration::for_type::<UpsertContainerEquipment>(),
            PacketRegistration::for_type::<EntityTooltipVersion>(),
            PacketRegistration::for_type::<EntityTooltip>(),
            PacketRegistration::for_type::<UpsertEntityStats>(),
            PacketRegistration::for_type::<RequestName>(),
            PacketRegistration::for_type::<RenameEntity>(),
            PacketRegistration::for_type::<EntityLightLevel>(),

            // Character
            PacketRegistration::for_type::<CharacterProfile>(),
            PacketRegistration::for_type::<Skills>(),
            PacketRegistration::for_type::<AttackRequest>(),
            PacketRegistration::for_type::<SetAttackTarget>(),
            PacketRegistration::for_type::<Swing>(),
            PacketRegistration::for_type::<DamageDealt>(),
            PacketRegistration::for_type::<CharacterAnimation>(),
            PacketRegistration::for_type::<CharacterPredefinedAnimation>(),
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

    pub fn encode(&self, client_version: ClientVersion, to_client: bool, writer: &mut impl Write)
        -> anyhow::Result<()> {
        (self.registration().encode)(client_version, to_client, writer, unsafe { transmute(&self.buffer) })
    }
}

impl Clone for AnyPacket {
    fn clone(&self) -> Self {
        unsafe {
            (self.registration().clone)(transmute(&self.buffer))
        }
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
    from_client: bool,
    encryption: Option<Encryption>,
}

impl Reader {
    pub fn set_encryption(&mut self, encryption: Option<Encryption>) {
        self.encryption = encryption;
    }

    pub async fn recv(&mut self, client_version: ClientVersion)
        -> anyhow::Result<AnyPacket> {
        let mut packet_kind = self.reader.read_u8().await?;

        if let Some(encryption) = self.encryption.as_mut() {
            let mut cell = [packet_kind];
            if self.from_client {
                encryption.crypt_client_to_server(&mut cell);
            } else {
                encryption.crypt_server_to_client(&mut cell);
            }
            packet_kind = cell[0];
        }

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
            let mut bytes = [0u8; 2];
            self.reader.read_exact(&mut bytes).await?;

            if let Some(encryption) = self.encryption.as_mut() {
                if self.from_client {
                    encryption.crypt_client_to_server(&mut bytes);
                } else {
                    encryption.crypt_server_to_client(&mut bytes);
                }
            }

            Endian::read_u16(&bytes) as usize - 3
        };

        log::trace!("Beginning {packet_kind:2x} length {length}");

        self.buffer.resize(length, 0u8);
        self.reader.read_exact(&mut self.buffer[..]).await?;

        if let Some(encryption) = self.encryption.as_mut() {
            if self.from_client {
                encryption.crypt_client_to_server(&mut self.buffer);
            } else {
                encryption.crypt_server_to_client(&mut self.buffer);
            }
        }

        let decoded = (registration.decode)(client_version, self.from_client, &self.buffer);
        self.buffer.clear();
        Ok(decoded?)
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
        self.writer.write_all(&mut addr_bytes).await?;
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
            }
        }

        let result = self.writer.write_all(&mut self.buffer).await;
        self.buffer.clear();
        result?;
        self.writer.flush().await?;
        Ok(())
    }

    pub async fn send<T: Packet>(&mut self, client_version: ClientVersion, packet: &T) -> anyhow::Result<()> {
        if let Some(length) = T::fixed_length(client_version) {
            self.buffer.reserve(length);
            self.buffer.push(T::packet_kind());
            packet.encode(client_version, self.to_client, &mut self.buffer)?;
            assert_eq!(length, self.buffer.len(), "Fixed length packet wrote wrong size");
        } else {
            self.buffer.extend([T::packet_kind(), 0, 0]);
            packet.encode(client_version, self.to_client, &mut self.buffer)?;
            let packet_len = self.buffer.len() as u16;
            Endian::write_u16(&mut self.buffer[1..3], packet_len);
        }

        self.send_raw().await
    }

    pub async fn send_any(&mut self, client_version: ClientVersion, packet: &AnyPacket) -> anyhow::Result<()> {
        if let Some(length) = packet.fixed_length(client_version) {
            self.buffer.reserve(length);
            self.buffer.push(packet.packet_kind());
            packet.encode(client_version, self.to_client, &mut self.buffer)?;
            assert_eq!(length, self.buffer.len(), "Fixed length packet wrote wrong size");
        } else {
            self.buffer.extend([packet.packet_kind(), 0, 0]);
            packet.encode(client_version, self.to_client, &mut self.buffer)?;
            let packet_len = self.buffer.len() as u16;
            Endian::write_u16(&mut self.buffer[1..3], packet_len);
        }

        self.send_raw().await
    }
}

pub fn new_io(stream: TcpStream, is_server: bool) -> (Reader, Writer) {
    let (reader, writer) = stream.into_split();
    (Reader {
        reader: BufReader::new(reader),
        buffer: Vec::with_capacity(4096),
        from_client: is_server,
        encryption: None,
    }, Writer {
        writer: BufWriter::new(writer),
        buffer: Vec::with_capacity(4096),
        has_sent: is_server,
        to_client: is_server,
        compress: false,
        compress_buffer: Vec::new(),
        encryption: None,
    })
}
