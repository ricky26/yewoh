use tokio::net::TcpListener;
use tokio::spawn;
use tokio::sync::mpsc;

use log::warn;
use yewoh::protocol::{ClientVersion, new_io};

use crate::world::client::NewConnection;

pub fn accept_player_connections(listener: TcpListener) -> mpsc::UnboundedReceiver<NewConnection> {
    let (mut tx, rx) = mpsc::unbounded_channel();

    spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, address)) => {
                    let (mut reader, mut writer) = new_io(stream, true);
                    let (packet_tx, mut to_send_rx) = mpsc::unbounded_channel();
                    let (mut received_tx, packet_rx) = mpsc::unbounded_channel();

                    spawn(async move {
                        let mut buffer = Vec::new();

                        loop {
                            match reader.receive(&mut buffer).await {
                                Ok(packet) => received_tx.send(packet).ok(),
                                Err(err) => {
                                    warn!("error receiving packet: {err}");
                                    break;
                                }
                            };
                        }
                    });

                    spawn(async move {
                        while let Some(packet) = to_send_rx.recv().await {
                            if let Err(err) =  writer.send_any(ClientVersion::default(), packet).await {
                                warn!("failed to send packet: {err}");
                                break;
                            }
                        }
                    });

                    tx.send(NewConnection{
                        address,
                        packet_rx,
                        packet_tx,
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
