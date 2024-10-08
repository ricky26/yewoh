use std::io::{Cursor, Read};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use crate::assets::uop::UopBuffer;

struct UopMul {
    uop: UopBuffer<Vec<u8>>,
    filename: String,
    next_block_index: usize,
    current_block: Cursor<Vec<u8>>,
}

enum MulReaderImpl {
    Uop(UopMul),
    Raw(Cursor<Vec<u8>>),
}

pub struct MulReader(MulReaderImpl);

impl Read for MulReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.0 {
            MulReaderImpl::Uop(uop) => {
                let position = uop.current_block.position() as usize;
                let inner = uop.current_block.get_mut();

                if position >= inner.len() {
                    // Read a new block
                    inner.clear();

                    let path = format!("build/{}legacymul/{:08}.dat", &uop.filename, uop.next_block_index);
                    if let Some(mut entry) = uop.uop.get(&path) {
                        uop.next_block_index += 1;
                        entry.read_to_end(inner).unwrap();
                        uop.current_block.set_position(0);
                    }
                }

                Read::read(&mut uop.current_block, buf)
            },
            MulReaderImpl::Raw(ref mut cursor) => Read::read(cursor, buf),
        }
    }
}

impl MulReader {
    pub async fn open(data_path: &Path, name: &str) -> anyhow::Result<MulReader> {
        let uop_path = data_path.join(format!("{}LegacyMUL.uop", name));
        if let Ok(mut file) = File::open(&uop_path).await {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).await?;
            let uop = UopBuffer::try_from_backing(contents)?;

            Ok(MulReader(MulReaderImpl::Uop(UopMul{
                uop,
                filename: name.to_string(),
                next_block_index: 0,
                current_block: Default::default(),
            })))
        } else {
            let path = data_path.join(format!("{}.mul", name));
            let mut file = File::open(&path).await?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).await?;
            Ok(MulReader(MulReaderImpl::Raw(Cursor::new(contents))))
        }
    }
}
