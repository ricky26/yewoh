use std::collections::HashSet;
use std::net::SocketAddr;

use bevy_ecs::prelude::*;
use tokio::sync::mpsc;

use yewoh::protocol::AnyPacket;

pub struct NewConnection {
    pub address: SocketAddr,
    pub packet_rx: mpsc::UnboundedReceiver<AnyPacket>,
    pub packet_tx: mpsc::UnboundedSender<AnyPacket>,
}

#[derive(Clone, Component)]
pub struct RemoteAddress {
    address: SocketAddr,
}

#[derive(Clone, Component)]
pub struct PacketSender {
    packet_tx: mpsc::UnboundedSender<AnyPacket>,
}

pub struct PlayerServer {
    new_connections: mpsc::UnboundedReceiver<NewConnection>,
    packet_rx: mpsc::UnboundedReceiver<(Entity, AnyPacket)>,

    internal_tx: mpsc::UnboundedSender<(Entity, AnyPacket)>,
    internal_close_tx: mpsc::UnboundedSender<Entity>,
    internal_close_rx: mpsc::UnboundedReceiver<Entity>,
}

impl PlayerServer {
    pub fn new(new_connections: mpsc::UnboundedReceiver<NewConnection>) -> PlayerServer {
        let (internal_tx, packet_rx) = mpsc::unbounded_channel();
        let (internal_close_tx, internal_close_rx) = mpsc::unbounded_channel();

        Self {
            new_connections,
            packet_rx,
            internal_tx,
            internal_close_tx,
            internal_close_rx,
        }
    }
}

impl Default for PlayerServer {
    fn default() -> Self {
        let (_, rx) = mpsc::unbounded_channel();
        PlayerServer::new(rx)
    }
}

pub struct PlayerClient {
    packet_tx: mpsc::Sender<()>,
}

pub fn accept_new_clients(mut server: ResMut<PlayerServer>, mut commands: Commands) {
    while let Ok(new_connection) = server.new_connections.try_recv() {
        let entity = commands.spawn()
            .insert(RemoteAddress { address: new_connection.address })
            .insert(PacketSender { packet_tx: new_connection.packet_tx })
            .id();

        let mut rx = new_connection.packet_rx;
        let mut internal_tx = server.internal_tx.clone();
        let mut internal_close = server.internal_close_tx.clone();
        tokio::spawn(async move {
            while let Some(packet) = rx.recv().await {
                if internal_tx.send((entity, packet)).is_err() {
                    return;
                }
            }

            internal_close.send(entity).ok();
        });
    }

    while let Ok(entity) = server.internal_close_rx.try_recv() {
        commands.entity(entity).despawn();
    }
}
