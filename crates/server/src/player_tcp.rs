use tokio::net::TcpListener;
use tokio::spawn;
use tokio::sync::mpsc;

use log::{info, warn};
use yewoh::protocol::{new_io};

use crate::world::client::NewConnection;

pub fn accept_player_connections(listener: TcpListener) -> mpsc::UnboundedReceiver<NewConnection> {
    let (tx, rx) = mpsc::unbounded_channel();

    spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, address)) => {
                    info!("New connection from {address}");
                    let (reader, writer) = new_io(stream, true);
                    tx.send(NewConnection{
                        address,
                        reader,
                        writer,
                    }).ok();
                }
                Err(error) => {
                    warn!("error listening for client connections: {error}");
                    break;
                }
            }
        }
    });

    rx
}
