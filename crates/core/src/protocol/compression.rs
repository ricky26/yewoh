use std::io::Write;

static HUFFMAN_ENCODE: [u32; 0x202] = [
    0x2, 0x000, 0x5, 0x01f, 0x6, 0x022, 0x7, 0x034, 0x7, 0x075, 0x6, 0x028, 0x6, 0x03b, 0x7, 0x032,
    0x8, 0x0e0, 0x8, 0x062, 0x7, 0x056, 0x8, 0x079, 0x9, 0x19d, 0x8, 0x097, 0x6, 0x02a, 0x7, 0x057,
    0x8, 0x071, 0x8, 0x05b, 0x9, 0x1cc, 0x8, 0x0a7, 0x7, 0x025, 0x7, 0x04f, 0x8, 0x066, 0x8, 0x07d,
    0x9, 0x191, 0x9, 0x1ce, 0x7, 0x03f, 0x9, 0x090, 0x8, 0x059, 0x8, 0x07b, 0x8, 0x091, 0x8, 0x0c6,
    0x6, 0x02d, 0x9, 0x186, 0x8, 0x06f, 0x9, 0x093, 0xa, 0x1cc, 0x8, 0x05a, 0xa, 0x1ae, 0xa, 0x1c0,
    0x9, 0x148, 0x9, 0x14a, 0x9, 0x082, 0xa, 0x19f, 0x9, 0x171, 0x9, 0x120, 0x9, 0x0e7, 0xa, 0x1f3,
    0x9, 0x14b, 0x9, 0x100, 0x9, 0x190, 0x6, 0x013, 0x9, 0x161, 0x9, 0x125, 0x9, 0x133, 0x9, 0x195,
    0x9, 0x173, 0x9, 0x1ca, 0x9, 0x086, 0x9, 0x1e9, 0x9, 0x0db, 0x9, 0x1ec, 0x9, 0x08b, 0x9, 0x085,
    0x5, 0x00a, 0x8, 0x096, 0x8, 0x09c, 0x9, 0x1c3, 0x9, 0x19c, 0x9, 0x08f, 0x9, 0x18f, 0x9, 0x091,
    0x9, 0x087, 0x9, 0x0c6, 0x9, 0x177, 0x9, 0x089, 0x9, 0x0d6, 0x9, 0x08c, 0x9, 0x1ee, 0x9, 0x1eb,
    0x9, 0x084, 0x9, 0x164, 0x9, 0x175, 0x9, 0x1cd, 0x8, 0x05e, 0x9, 0x088, 0x9, 0x12b, 0x9, 0x172,
    0x9, 0x10a, 0x9, 0x08d, 0x9, 0x13a, 0x9, 0x11c, 0xa, 0x1e1, 0xa, 0x1e0, 0x9, 0x187, 0xa, 0x1dc,
    0xa, 0x1df, 0x7, 0x074, 0x9, 0x19f, 0x8, 0x08d, 0x8, 0x0e4, 0x7, 0x079, 0x9, 0x0ea, 0x9, 0x0e1,
    0x8, 0x040, 0x7, 0x041, 0x9, 0x10b, 0x9, 0x0b0, 0x8, 0x06a, 0x8, 0x0c1, 0x7, 0x071, 0x7, 0x078,
    0x8, 0x0b1, 0x9, 0x14c, 0x7, 0x043, 0x8, 0x076, 0x7, 0x066, 0x7, 0x04d, 0x9, 0x08a, 0x6, 0x02f,
    0x8, 0x0c9, 0x9, 0x0ce, 0x9, 0x149, 0x9, 0x160, 0xa, 0x1ba, 0xa, 0x19e, 0xa, 0x39f, 0x9, 0x0e5,
    0x9, 0x194, 0x9, 0x184, 0x9, 0x126, 0x7, 0x030, 0x8, 0x06c, 0x9, 0x121, 0x9, 0x1e8, 0xa, 0x1c1,
    0xa, 0x11d, 0xa, 0x163, 0xa, 0x385, 0xa, 0x3db, 0xa, 0x17d, 0xa, 0x106, 0xa, 0x397, 0xa, 0x24e,
    0x7, 0x02e, 0x8, 0x098, 0xa, 0x33c, 0xa, 0x32e, 0xa, 0x1e9, 0x9, 0x0bf, 0xa, 0x3df, 0xa, 0x1dd,
    0xa, 0x32d, 0xa, 0x2ed, 0xa, 0x30b, 0xa, 0x107, 0xa, 0x2e8, 0xa, 0x3de, 0xa, 0x125, 0xa, 0x1e8,
    0x9, 0x0e9, 0xa, 0x1cd, 0xa, 0x1b5, 0x9, 0x165, 0xa, 0x232, 0xa, 0x2e1, 0xb, 0x3ae, 0xb, 0x3c6,
    0xb, 0x3e2, 0xa, 0x205, 0xa, 0x29a, 0xa, 0x248, 0xa, 0x2cd, 0xa, 0x23b, 0xb, 0x3c5, 0xa, 0x251,
    0xa, 0x2e9, 0xa, 0x252, 0x9, 0x1ea, 0xb, 0x3a0, 0xb, 0x391, 0xa, 0x23c, 0xb, 0x392, 0xb, 0x3d5,
    0xa, 0x233, 0xa, 0x2cc, 0xb, 0x390, 0xa, 0x1bb, 0xb, 0x3a1, 0xb, 0x3c4, 0xa, 0x211, 0xa, 0x203,
    0x9, 0x12a, 0xa, 0x231, 0xb, 0x3e0, 0xa, 0x29b, 0xb, 0x3d7, 0xa, 0x202, 0xb, 0x3ad, 0xa, 0x213,
    0xa, 0x253, 0xa, 0x32c, 0xa, 0x23d, 0xa, 0x23f, 0xa, 0x32f, 0xa, 0x11c, 0xa, 0x384, 0xa, 0x31c,
    0xa, 0x17c, 0xa, 0x30a, 0xa, 0x2e0, 0xa, 0x276, 0xa, 0x250, 0xb, 0x3e3, 0xa, 0x396, 0xa, 0x18f,
    0xa, 0x204, 0xa, 0x206, 0xa, 0x230, 0xa, 0x265, 0xa, 0x212, 0xa, 0x23e, 0xb, 0x3ac, 0xb, 0x393,
    0xb, 0x3e1, 0xa, 0x1de, 0xb, 0x3d6, 0xa, 0x31d, 0xb, 0x3e5, 0xb, 0x3e4, 0xa, 0x207, 0xb, 0x3c7,
    0xa, 0x277, 0xb, 0x3d4, 0x8, 0x0c0, 0xa, 0x162, 0xa, 0x3da, 0xa, 0x124, 0xa, 0x1b4, 0xa, 0x264,
    0xa, 0x33d, 0xa, 0x1d1, 0xa, 0x1af, 0xa, 0x39e, 0xa, 0x24f, 0xb, 0x373, 0xa, 0x249, 0xb, 0x372,
    0x9, 0x167, 0xa, 0x210, 0xa, 0x23a, 0xa, 0x1b8, 0xb, 0x3af, 0xa, 0x18e, 0xa, 0x2ec, 0x7, 0x062,
    0x4, 0x00d
];

static HUFFMAN_DECODE: [i16; 0x200] = [
    2, 1, 4, 3, 0, 5, 7, 6, 9, 8, 11, 10, 13, 12, 14, -256, 16, 15, 18, 17, 20, 19, 22, 21, 23, -1,
    25, 24, 27, 26, 29, 28, 31, 30, 33, 32, 35, 34, 37, 36, 39, 38, -64, 40, 42, 41, 44, 43, 45, -6,
    47, 46, 49, 48, 51, 50, 52, -119, 53, -32, -14, 54, -5, 55, 57, 56, 59, 58, -2, 60, 62, 61, 64,
    63, 66, 65, 68, 67, 70, 69, 72, 71, 73, -51, 75, 74, 77, 76, -111, -101, -97, -4, 79, 78, 80,
    -110, -116, 81, 83, 82, -255, 84, 86, 85, 88, 87, 90, 89, -10, -15, 92, 91, 93, -21, 94, -117,
    96, 95, 98, 97, 100, 99, 101, -114, 102, -105, 103, -26, 105, 104, 107, 106, 109, 108, 111, 110,
    -3, 112, -7, 113, -131, 114, -144, 115, 117, 116, 118, -20, 120, 119, 122, 121, 124, 123, 126,
    125, 128, 127, -100, 129, -8, 130, 132, 131, 134, 133, 135, -120, -31, 136, 138, 137, -234,
    -109, 140, 139, 142, 141, 144, 143, 145, -112, 146, -19, 148, 147, -66, 149, -145, 150, -65,
    -13, 152, 151, 154, 153, 155, -30, 157, 156, 158, -99, 160, 159, 162, 161, 163, -23, 164, -29,
    165, -11, -115, 166, 168, 167, 170, 169, 171, -16, 172, -34, -132, 173, -108, 174, -22, 175, -9,
    176, -84, 177, -37, -17, 178, -28, 180, 179, 182, 181, 184, 183, 186, 185, -104, 187, -78, 188,
    -61, 189, -178, -79, -134, -59, -25, 190, -18, -83, -57, 191, 192, -67, 193, -98, -68, -12, 195,
    194, -128, -55, -50, -24, 196, -70, -33, -94, -129, 197, 198, -74, 199, -82, -87, -56, 200, -44,
    201, -248, -81, -163, -123, -52, -113, 202, -41, -48, -40, -122, -90, 203, 204, -54, -192, -86,
    206, 205, -130, 207, 208, -53, -45, -133, 210, 209, -91, 211, 213, 212, -88, -106, 215, 214,
    217, 216, -49, 218, 220, 219, 222, 221, 224, 223, 226, 225, -102, 227, 228, -160, 229, -46, 230,
    -127, 231, -103, 233, 232, 234, -60, -76, 235, -121, 236, -73, 237, 238, -149, -107, 239, 240,
    -35, -27, -71, 241, -69, -77, -89, -118, -62, -85, -75, -58, -72, -80, -63, -42, 242, -157,
    -150, -236, -139, -243, -126, -214, -142, -206, -138, -146, -240, -147, -204, -201, -152, -207,
    -227, -209, -154, -254, -153, -156, -176, -210, -165, -185, -172, -170, -195, -211, -232, -239,
    -219, -177, -200, -212, -175, -143, -244, -171, -246, -221, -203, -181, -202, -250, -173, -164,
    -184, -218, -193, -220, -199, -249, -190, -217, -230, -216, -169, -197, -191, 243, -47, 245,
    244, 247, 246, -159, -148, 249, 248, -93, -92, -225, -96, -95, -151, 251, 250, 252, -241, -36,
    -161, 254, 253, -39, -135, -124, -187, -251, 255, -238, -162, -38, -242, -125, -43, -253, -215,
    -208, -140, -235, -137, -237, -158, -205, -136, -141, -155, -229, -228, -168, -213, -194, -224,
    -226, -196, -233, -183, -167, -231, -189, -174, -166, -252, -222, -198, -179, -188, -182, -223,
    -186, -180, -247, -245,
];

struct BitWriter<'a> {
    storage: &'a mut Vec<u8>,
    buffer: u32,
    buffer_length: u32,
}

impl<'a> BitWriter<'a> {
    pub fn new(storage: &'a mut Vec<u8>) -> BitWriter<'a> {
        Self {
            storage,
            buffer: 0,
            buffer_length: 0,
        }
    }

    #[inline]
    fn write(&mut self) {
        while self.buffer_length >= 8 {
            self.buffer_length -= 8;
            let value = ((self.buffer >> self.buffer_length) & 0xff) as u8;
            self.storage.push(value);
        }
    }

    #[inline]
    pub fn write_bits(&mut self, n: u32, v: u32) {
        self.buffer_length += n;
        self.buffer = self.buffer.wrapping_shl(n) | v;
        assert!(self.buffer_length <= 32);
        self.write();
    }

    #[inline]
    pub fn flush(&mut self) {
        if self.buffer_length & 7 != 0 {
            let align = 8 - (self.buffer_length & 7);
            self.buffer_length += align;
            self.buffer <<= align;
        }

        self.write();
    }
}

pub struct BitReader<'a> {
    src: &'a [u8],
    bit_offset: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(src: &'a [u8]) -> BitReader<'a> {
        Self {
            src,
            bit_offset: 0,
        }
    }

    #[inline]
    pub fn pop(&mut self) -> Option<bool> {
        if self.src.len() > 0 {
            let value = (self.src[0] & (1 << (7 - self.bit_offset))) != 0;

            if self.bit_offset >= 7 {
                self.bit_offset = 0;
                self.src = &self.src[1..];
            } else {
                self.bit_offset += 1;
            }

            Some(value)
        } else {
            None
        }
    }

    #[inline]
    pub fn flush_byte(&mut self) {
        if self.bit_offset != 0 {
            self.src = &self.src[1..];
            self.bit_offset = 0;
        }
    }
}

pub fn huffman_compress(src: &[u8], dest: &mut Vec<u8>) {
    let mut writer = BitWriter::new(dest);

    for i in src.iter().copied() {
        let i = i as usize;
        writer.write_bits(HUFFMAN_ENCODE[i << 1], HUFFMAN_ENCODE[(i << 1) + 1]);
    }

    writer.write_bits(HUFFMAN_ENCODE[0x200], HUFFMAN_ENCODE[0x201]);
    writer.flush();
}

pub struct HuffmanVecWriter<'a> {
    storage: BitWriter<'a>,
}

impl<'a> HuffmanVecWriter<'a> {
    pub fn new(storage: &'a mut Vec<u8>) -> HuffmanVecWriter {
        Self {
            storage: BitWriter::new(storage),
        }
    }

    pub fn finish(&mut self) {
        self.storage.write_bits(HUFFMAN_ENCODE[0x200], HUFFMAN_ENCODE[0x201]);
        self.storage.flush();
    }
}

impl<'a> Write for HuffmanVecWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for i in buf {
            let i = *i as usize;
            self.storage.write_bits(HUFFMAN_ENCODE[i << 1], HUFFMAN_ENCODE[(i << 1) + 1]);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct HuffmanDecoder {
    storage: Vec<u8>,
    entry_idx: Option<usize>,
}

impl HuffmanDecoder {
    #[inline]
    pub fn write(&mut self, bytes: &[u8]) -> Option<(usize, Vec<u8>)> {
        self.storage.reserve(bytes.len());

        let mut reader = BitReader::new(bytes);

        while let Some(bit) = reader.pop() {
            let next = if let Some(entry_idx) = self.entry_idx {
                HUFFMAN_DECODE[if bit { entry_idx + 1 } else { entry_idx }]
            } else {
                HUFFMAN_DECODE[if bit { 1 } else { 0 }]
            };

            if next <= -256 {
                reader.flush_byte();
                return Some((bytes.len() - reader.src.len(), std::mem::take(&mut self.storage)));
            }

            if next < 1 {
                self.storage.push((-next) as u8);
                self.entry_idx = None;
            } else {
                self.entry_idx = Some((next as usize) * 2);
            }
        }

        None
    }
}