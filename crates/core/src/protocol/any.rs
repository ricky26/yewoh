use std::any::type_name;
use std::io::Write;
use std::sync::Arc;
use crate::protocol::{AccountLogin, AsciiTextMessage, AsciiTextMessageRequest, AttackRequest, BeginEnterWorld, ChangeSeason, CharacterAnimation, CharacterList, CharacterPredefinedAnimation, CharacterProfile, ClientVersion, ClientVersionRequest, CreateCharacterClassic, CreateCharacterEnhanced, DamageDealt, DeleteCharacter, DeleteEntity, DoubleClick, DropEntity, EndEnterWorld, EntityLightLevel, EntityRequest, EntityTooltip, EntityTooltipVersion, EquipEntity, ExtendedCommand, ExtendedCommandAos, GameServerLogin, GlobalLightLevel, GumpResult, LocalisedTextMessage, LoginError, Logout, Move, MoveConfirm, MoveEntityReject, MoveReject, OpenChatWindow, OpenContainer, OpenGump, OpenGumpCompressed, OpenPaperDoll, OutgoingPacket, Packet, PickTarget, PickUpEntity, Ping, PlayMusic, PlaySoundEffect, RenameEntity, RequestHelp, RequestName, Seed, SelectCharacter, SelectGameServer, ServerList, SetAttackTarget, SetTime, ShowPublicHouses, SingleClick, Skills, SupportedFeatures, Swing, SwitchServer, UnicodeTextMessage, UnicodeTextMessageRequest, UpdateCharacter, UpsertContainerContents, UpsertContainerEquipment, UpsertEntityCharacter, UpsertEntityContained, UpsertEntityEquipped, UpsertEntityLegacy, UpsertEntityStats, UpsertEntityWorld, UpsertLocalPlayer, ViewRange, WarMode};

pub trait IntoAnyPacket where Self: Sized {
    fn into_any(self) -> AnyPacket;

    fn into_any_arc(self) -> Arc<AnyPacket> {
        Arc::new(self.into_any())
    }

    fn into_any_maybe_arc(self) -> Result<AnyPacket, Arc<AnyPacket>> {
        Ok(self.into_any())
    }
}

pub trait AnyDowncast where Self: Sized {
    #[allow(clippy::result_large_err)]
    fn downcast(src: AnyPacket) -> Result<Self, AnyPacket>;
    fn downcast_ref(src: &AnyPacket) -> Option<&Self>;
    fn downcast_mut(src: &mut AnyPacket) -> Option<&mut Self>;
}

pub(crate) struct PacketRegistration {
    pub type_name: &'static str,
    pub fixed_length: fn(client_version: ClientVersion) -> Option<usize>,
    pub decode: fn(client_version: ClientVersion, from_client: bool, payload: &[u8]) -> anyhow::Result<AnyPacket>,
}

impl AnyPacket {
    fn packet_registration<P: Packet + Into<AnyPacket>>() -> PacketRegistration {
        PacketRegistration {
            type_name: type_name::<P>(),
            fixed_length: P::fixed_length,
            decode: |client_version, from_client, payload| {
                P::decode(client_version, from_client, payload).map(Into::into)
            },
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn downcast<T: AnyDowncast>(self) -> Result<T, AnyPacket> {
        T::downcast(self)
    }

    pub fn downcast_ref<T: AnyDowncast>(&self) -> Option<&T> {
        T::downcast_ref(self)
    }

    pub fn downcast_mut<T: AnyDowncast>(&mut self) -> Option<&mut T> {
        T::downcast_mut(self)
    }
}

impl IntoAnyPacket for AnyPacket {
    fn into_any(self) -> AnyPacket {
        self
    }
}

impl IntoAnyPacket for Arc<AnyPacket> {
    fn into_any(self) -> AnyPacket {
        (*self).clone()
    }

    fn into_any_arc(self) -> Arc<AnyPacket> {
        self
    }

    fn into_any_maybe_arc(self) -> Result<AnyPacket, Arc<AnyPacket>> {
        Err(self)
    }
}

macro_rules! impl_packet {
    ($ty:ident) => {
        impl From<$ty> for AnyPacket {
            fn from(packet: $ty) -> AnyPacket {
                AnyPacket::$ty(packet)
            }
        }

        impl IntoAnyPacket for $ty {
            fn into_any(self) -> AnyPacket {
                AnyPacket::$ty(self)
            }
        }

        impl AnyDowncast for $ty {
            fn downcast(src: AnyPacket) -> Result<Self, AnyPacket> {
                match src {
                    AnyPacket::$ty(p) => Ok(p),
                    v => Err(v),
                }
            }

            fn downcast_ref(src: &AnyPacket) -> Option<&Self> {
                match src {
                    AnyPacket::$ty(p) => Some(p),
                    _ => None,
                }
            }

            fn downcast_mut(src: &mut AnyPacket) -> Option<&mut Self> {
                match src {
                    AnyPacket::$ty(p) => Some(p),
                    _ => None,
                }
            }
        }
    };
}

macro_rules! impl_any {
    ($($ty:ident),+ $(,)?) => {
        #[derive(Clone, Debug)]
        pub enum AnyPacket {
            $($ty($ty),)+
        }

        impl AnyPacket {
            pub(crate) fn registration_for(packet_kind: u8) -> Option<PacketRegistration> {
                match packet_kind {
                    $($ty::PACKET_KIND => Some(Self::packet_registration::<$ty>()),)*
                    _ => None,
                }
            }
        }

        impl OutgoingPacket for AnyPacket {
            fn packet_type_name(&self) -> &'static str {
                match self {
                    $(AnyPacket::$ty(p) => p.packet_type_name(),)*
                }
            }

            fn packet_kind(&self) -> u8 {
                match self {
                    $(AnyPacket::$ty(p) => p.packet_kind(),)*
                }
            }

            fn fixed_length(&self, client_version: ClientVersion) -> Option<usize> {
                match self {
                    $(AnyPacket::$ty(p) => p.fixed_length(client_version),)*
                }
            }

            fn encode(
                &self, client_version: ClientVersion, to_client: bool, writer: &mut impl Write,
            ) -> anyhow::Result<()> {
                match self {
                    $(AnyPacket::$ty(p) => OutgoingPacket::encode(p, client_version, to_client, writer),)*
                }
            }
        }

        $(impl_packet!($ty);)*
    }
}

impl_any!(
    // Add packet types here.

    // Login
    Seed,
    AccountLogin,
    LoginError,
    ServerList,
    SelectGameServer,
    SwitchServer,
    GameServerLogin,
    SupportedFeatures,
    CharacterList,
    CreateCharacterClassic,
    CreateCharacterEnhanced,
    DeleteCharacter,
    SelectCharacter,
    ClientVersionRequest,
    BeginEnterWorld,
    EndEnterWorld,
    ShowPublicHouses,
    Ping,
    Logout,
    WarMode,
    RequestHelp,

    // Extended
    ExtendedCommand,
    ExtendedCommandAos,

    // Input
    Move,
    MoveConfirm,
    MoveReject,
    SingleClick,
    DoubleClick,
    PickUpEntity,
    DropEntity,
    MoveEntityReject,
    EquipEntity,
    PickTarget,

    // UI
    OpenChatWindow,
    OpenPaperDoll,
    OpenContainer,
    OpenGump,
    OpenGumpCompressed,
    GumpResult,

    // Chat
    AsciiTextMessage,
    UnicodeTextMessage,
    LocalisedTextMessage,
    AsciiTextMessageRequest,
    UnicodeTextMessageRequest,

    // Sound
    PlayMusic,
    PlaySoundEffect,

    // Map
    SetTime,
    ChangeSeason,
    ViewRange,
    GlobalLightLevel,

    // Entity
    EntityRequest,
    UpsertEntityLegacy,
    UpsertEntityWorld,
    DeleteEntity,
    UpsertLocalPlayer,
    UpsertEntityCharacter,
    UpdateCharacter,
    UpsertEntityEquipped,
    UpsertEntityContained,
    UpsertContainerContents,
    UpsertContainerEquipment,
    EntityTooltipVersion,
    EntityTooltip,
    UpsertEntityStats,
    RequestName,
    RenameEntity,
    EntityLightLevel,

    // Character
    CharacterProfile,
    Skills,
    AttackRequest,
    SetAttackTarget,
    Swing,
    DamageDealt,
    CharacterAnimation,
    CharacterPredefinedAnimation,
);
