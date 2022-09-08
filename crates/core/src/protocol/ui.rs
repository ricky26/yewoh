use std::io::Write;

use crate::protocol::PacketWriteExt;

use super::{ClientVersion, Packet};

#[derive(Debug, Clone)]
pub struct OpenChatWindow;

impl Packet for OpenChatWindow {
    fn packet_kind() -> u8 { 0xb5 }
    fn fixed_length(_client_version: ClientVersion) -> Option<usize> { Some(64) }

    fn decode(_client_version: ClientVersion, _payload: &[u8]) -> anyhow::Result<Self> {
        Ok(Self)
    }

    fn encode(&self, _client_version: ClientVersion, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_zeros(63)?;
        Ok(())
    }
}
