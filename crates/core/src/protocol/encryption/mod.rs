use crate::protocol::ClientVersion;
use crate::protocol::encryption::blowfish_pass::BlowfishPass;
use crate::protocol::encryption::lobby_pass::LobbyPass;
use crate::protocol::encryption::twofish_pass::TwofishPass;

mod blowfish;
mod twofish;

mod lobby_pass;
mod blowfish_pass;
mod twofish_pass;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EncryptionKind {
    BlowFishV1,
    BlowFishV2,
    BlowFishV3,
    BlowFishV4,
    TwoFish,
}

impl EncryptionKind {
    pub fn for_version(client_version: ClientVersion) -> EncryptionKind {
        if client_version > ClientVersion::new(2, 0, 3, 0) {
            EncryptionKind::TwoFish
        } else if client_version > ClientVersion::new(2, 0, 0, 0) {
            EncryptionKind::BlowFishV4
        } else if client_version == ClientVersion::new(1, 25, 36, 0) {
            EncryptionKind::BlowFishV2
        } else if client_version >= ClientVersion::new(1, 25, 35, 0) {
            EncryptionKind::BlowFishV3
        } else {
            EncryptionKind::BlowFishV1
        }
    }
}

#[derive(Clone)]
pub struct GameEncryption {
    kind: EncryptionKind,
    blowfish: BlowfishPass,
    twofish: TwofishPass,
}

impl GameEncryption {
    pub fn new(kind: EncryptionKind, seed: u32) -> GameEncryption {
        let blowfish = BlowfishPass::new();
        let twofish = TwofishPass::new(seed);
        Self { kind, blowfish, twofish }
    }

    pub fn crypt_client_to_server(&mut self, data: &mut [u8]) {
        if self.kind != EncryptionKind::TwoFish {
            self.blowfish.crypt(data);
        }

        if self.kind == EncryptionKind::BlowFishV4 || self.kind == EncryptionKind::TwoFish {
            self.twofish.crypt_client_to_server(data);
        }
    }

    pub fn crypt_server_to_client(&mut self, data: &mut [u8]) {
        if self.kind == EncryptionKind::TwoFish {
            self.twofish.crypt_server_to_client(data);
        }
    }
}

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Encryption {
    Lobby(LobbyPass),
    Game(GameEncryption),
}

impl Encryption {
    pub fn new(client_version: ClientVersion, seed: u32, is_lobby: bool) -> Encryption {
        let kind = EncryptionKind::for_version(client_version);
        if is_lobby {
            Encryption::Lobby(LobbyPass::new(kind.into(), client_version, seed))
        } else {
            Encryption::Game(GameEncryption::new(kind, seed))
        }
    }

    pub fn crypt_client_to_server(&mut self, data: &mut [u8]) {
        match self {
            Encryption::Lobby(pass) => pass.encrypt(data),
            Encryption::Game(pass) => pass.crypt_client_to_server(data),
        }
    }

    pub fn crypt_server_to_client(&mut self, data: &mut [u8]) {
        match self {
            Encryption::Lobby(_) => {}
            Encryption::Game(pass) => pass.crypt_server_to_client(data),
        }
    }
}

pub struct EncryptionReader {

}
