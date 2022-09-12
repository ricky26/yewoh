use crate::protocol::ClientVersion;
use super::EncryptionKind;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LobbyEncryptionKind {
    V1,
    V2,
    V3,
}

impl From<EncryptionKind> for LobbyEncryptionKind {
    fn from(kind: EncryptionKind) -> Self {
        match kind {
            EncryptionKind::BlowFishV1 => LobbyEncryptionKind::V1,
            EncryptionKind::BlowFishV2 => LobbyEncryptionKind::V2,
            _ => LobbyEncryptionKind::V3,
        }
    }
}

#[derive(Clone)]
pub struct LobbyPass {
    kind: LobbyEncryptionKind,
    keys: [u32; 3],
    state: [u32; 2],
}

impl LobbyPass {
    pub fn new(kind: LobbyEncryptionKind, client_version: ClientVersion, seed: u32) -> LobbyPass {
        let a = client_version.major as u32;
        let b = client_version.minor as u32;
        let c = client_version.patch as u32;

        let first = ((((a << 9) | b) << 10) | c) ^ ((c * c) << 5);
        let key_2 = (first << 4) ^ (b * b) ^ (b * 0x0B000000) ^ (c * 0x380000) ^ 0x2C13A5FD;
        let second = (((((a << 9) | c) << 10) | b) * 8) ^ (c * c * 0x0c00);
        let key_3 = second ^ (b * b) ^ (b * 0x6800000) ^ (c * 0x1c0000) ^ 0x0A31D527F;
        let key_1 = key_2 - 1;
        let keys = [key_1, key_2, key_3];
        let state = [
            ((!seed ^ 0x1357) << 16) | ((seed ^ 0xaaaa) & 0xffff),
            ((seed >> 16) ^ 0x4321) | ((!seed ^ 0xabcd0000) & 0xffff0000),
        ];

        Self { kind, keys, state }
    }

    pub fn encrypt_one(&mut self, src: u8) -> u8 {
        let state = self.state;
        let result = src ^ (state[0] as u8);

        match self.kind {
            LobbyEncryptionKind::V1 => {
                self.state = [
                    ((state[0] >> 1) | (state[1] << 31)) ^ self.keys[1],
                    ((state[1] >> 1) | (state[0] << 31)) ^ self.keys[0],
                ];
            }
            LobbyEncryptionKind::V2 => {
                let second = (self.keys[0] >> (5 * state[1] * state[1]))
                    + state[1] * self.keys[0]
                    + state[0] * state[0] * 0x35ce9581
                    + 0x07afcc37;
                let first = (self.keys[1] >> (3 * state[0] * state[0]))
                    + state[0] * self.keys[1]
                    + second * second * 0x4c3a1353
                    + 0x16ef783f;
                self.state = [first, second];
            }
            LobbyEncryptionKind::V3 => {
                self.state = [
                    ((state[0] >> 1) | (state[1] << 31)) ^ self.keys[2],
                    (((((state[1] >> 1) | (state[0] << 31)) ^ self.keys[0]) >> 1) | (state[0] << 31)) ^ self.keys[1],
                ];
            }
        }

        result
    }

    pub fn encrypt(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            *byte = self.encrypt_one(*byte);
        }
    }
}
