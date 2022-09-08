use std::collections::HashSet;
use std::net::SocketAddr;

use bevy_ecs::prelude::*;
use glam::IVec3;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use yewoh::protocol::{AccountLogin, AnyPacket, CharacterList, GameServer, Packet, SelectGameServer, ServerList, StartingCity};

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

pub fn accept_new_clients(runtime: Res<Handle>, mut server: ResMut<PlayerServer>,
    mut commands: Commands) {
    while let Ok(new_connection) = server.new_connections.try_recv() {
        let entity = commands.spawn()
            .insert(RemoteAddress { address: new_connection.address })
            .insert(PacketSender { packet_tx: new_connection.packet_tx })
            .id();

        let mut rx = new_connection.packet_rx;
        let mut internal_tx = server.internal_tx.clone();
        let mut internal_close = server.internal_close_tx.clone();
        runtime.spawn(async move {
            while let Some(packet) = rx.recv().await {
                if let Err(err) = internal_tx.send((entity, packet)) {
                    log::warn!("Error forwarding packet {err}");
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

pub fn handle_packets(mut server: ResMut<PlayerServer>, connections: Query<(&PacketSender)>) {
    while let Ok((entity, packet)) = server.packet_rx.try_recv() {
        let sender = match connections.get(entity) {
            Ok(x) => x,
            _ => continue,
        };

        if let Some(account_login) = packet.downcast::<AccountLogin>() {
            log::info!("New login attempt for {}", &account_login.username);
            sender.packet_tx.send(ServerList {
                system_info_flags: 0x5d,
                game_servers: vec![
                    GameServer {
                        server_name: "My Server".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }.into()).ok();
        } else if let Some(game_server) = packet.downcast::<SelectGameServer>() {
            sender.packet_tx.send(CharacterList {
                characters: vec![None; 5],
                cities: vec![StartingCity {
                    index: 0,
                    city: "My City".into(),
                    building: "My Building".into(),
                    location: IVec3::new(0, 1, 2),
                    map_id: 0,
                    description_id: 0,
                }],
            }.into()).ok();
        }
    }
}
