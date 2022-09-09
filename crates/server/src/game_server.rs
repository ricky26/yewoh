use std::net::SocketAddr;

use anyhow::anyhow;
use log::{info, warn};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::spawn;
use tokio::sync::mpsc;

use yewoh::protocol::{ClientVersion, GameServerLogin, new_io, Reader, Writer};

pub struct NewSessionAttempt {
    pub address: SocketAddr,
    pub reader: Reader,
    pub writer: Writer,
    pub token: u32,
    pub username: String,
    pub password: String,
}

pub async fn serve_game(
    mut stream: TcpStream,
    address: SocketAddr,
    tx: mpsc::UnboundedSender<NewSessionAttempt>,
) -> anyhow::Result<()> {
    let token = stream.read_u32().await?;

    let (mut reader, mut writer) = new_io(stream, true);
    let login = reader.recv(ClientVersion::default()).await?
        .into_downcast::<GameServerLogin>()
        .ok()
        .ok_or_else(|| anyhow!("expected game server login as first packet"))?;

    if login.token != token {
        return Err(anyhow!("expected initial token & login token to match"));
    }

    writer.enable_compression();
    tx
        .send(NewSessionAttempt {
            address,
            reader,
            writer,
            token,
            username: login.username,
            password: login.password,
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
