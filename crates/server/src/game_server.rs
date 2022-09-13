use std::net::SocketAddr;

use anyhow::anyhow;
use log::{info, warn};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::spawn;
use tokio::sync::mpsc;

use yewoh::protocol::{new_io, Reader, Writer};

pub struct NewSessionAttempt {
    pub address: SocketAddr,
    pub reader: Reader,
    pub writer: Writer,
    pub token: u32,
}

pub async fn serve_game(
    mut stream: TcpStream,
    address: SocketAddr,
    tx: mpsc::UnboundedSender<NewSessionAttempt>,
) -> anyhow::Result<()> {
    let token = stream.read_u32().await?;
    let (reader, mut writer) = new_io(stream, true);
    writer.enable_compression();

    tx
        .send(NewSessionAttempt {
            address,
            reader,
            writer,
            token,
        })
        .map_err(|_| anyhow!("failed to start new session"))?;

    Ok(())
}

pub async fn listen_for_game(
    listener: TcpListener,
    tx: mpsc::UnboundedSender<NewSessionAttempt>,
) -> anyhow::Result<()> {
    loop {
        let (stream, address) = listener.accept().await?;
        info!("New game connection from {address}");
        let tx = tx.clone();
        spawn(async move {
            if let Err(err) = serve_game(stream, address, tx).await {
                warn!("Error serving game: {:?}", err);
            }
        });
    }
}
