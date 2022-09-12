use super::twofish::Twofish;
use byteorder::{BigEndian as Endian, ByteOrder};

const BLOCK_SIZE: usize = 256;

#[derive(Clone)]
pub struct TwofishPass {
    twofish: Twofish,
    block: [u8; BLOCK_SIZE],
    block_offset: usize,
    response_block: [u8; 16],
}

impl TwofishPass {
    pub fn new(seed: u32) -> TwofishPass {
        let mut key = [0u8; 16];
        Endian::write_u32_into(&[seed, seed, seed, seed], &mut key);
        let mut twofish = Twofish::new();
        twofish.key_schedule(&key);

        let mut block = [0u8; BLOCK_SIZE];
        for (idx, x) in block.iter_mut().enumerate() {
            *x = idx as u8;
        }

        Self::rotate_block(&mut twofish, &mut block);

        let response_block = md5::compute(&block).0;
        Self {
            twofish,
            block,
            block_offset: 0,
            response_block,
        }
    }

    fn rotate_block(twofish: &mut Twofish, block: &mut [u8; BLOCK_SIZE]) {
        for chunk in block.chunks_mut(16) {
            twofish.encrypt(chunk);
        }
    }

    fn ensure_block(&mut self) {
        if self.block_offset < BLOCK_SIZE {
            return;
        }

        Self::rotate_block(&mut self.twofish, &mut self.block);
        self.block_offset = 0;
    }

    pub fn crypt_server_to_client(&mut self, data: &mut [u8]) {
        for x in data.iter_mut() {
            *x ^= self.response_block[self.block_offset & 0xf];
            self.block_offset += 1;
        }
    }

    pub fn crypt_client_to_server(&mut self, data: &mut [u8]) {
        for x in data.iter_mut() {
            self.ensure_block();
            *x ^= self.block[self.block_offset];
            self.block_offset += 1;
        }
    }
}