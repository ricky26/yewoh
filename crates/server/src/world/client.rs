use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};

use bevy_ecs::prelude::*;
use glam::IVec3;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use yewoh::protocol::{AccountLogin, AnyPacket, CharacterList, FeatureFlags, GameServer, GameServerLogin, Packet, Reader, SelectGameServer, ServerList, StartingCity, SupportedFeatures, SwitchServer, Writer};

pub struct NewConnection {
    pub address: SocketAddr,
    pub reader: Reader,
    pub writer: Writer,
}

#[derive(Clone, Component)]
pub struct RemoteAddress {
    address: SocketAddr,
}

#[derive(Clone, Component)]
pub struct PacketSender {
    writer: Writer,
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
            .insert(PacketSender { writer: new_connection.writer })
            .id();

        let mut reader = new_connection.reader;
        let mut internal_tx = server.internal_tx.clone();
        let mut internal_close = server.internal_close_tx.clone();
        runtime.spawn(async move {
            let client_version = Default::default();

            while let Ok(packet) = reader.receive(client_version).await {
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

pub fn handle_packets(mut server: ResMut<PlayerServer>, mut connections: Query<(&mut PacketSender)>) {
    while let Ok((entity, packet)) = server.packet_rx.try_recv() {
        let sender = match connections.get_mut(entity) {
            Ok(x) => x,
            _ => continue,
        };

        log::debug!("Got packet type {:2x}", packet.packet_kind());

        let client_version = Default::default();
        if let Some(account_login) = packet.downcast::<AccountLogin>() {
            log::info!("New login attempt for {}", &account_login.username);
            sender.writer.send(client_version, &ServerList {
                system_info_flags: 0x5d,
                game_servers: vec![
                    GameServer {
                        server_name: "My Server".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }).ok();
        } else if let Some(game_server) = packet.downcast::<SelectGameServer>() {
            sender.writer.send(client_version, &SwitchServer {
                ip: Ipv4Addr::LOCALHOST.into(),
                port: 2593,
                token: 7,
            }).ok();
        } else if let Some(game_login) = packet.downcast::<GameServerLogin>() {
            log::debug!("token {}", game_login.seed);
            sender.writer.send(client_version, &SupportedFeatures {
                feature_flags: FeatureFlags::empty(),
            }.into()).ok();
            sender.writer.send(client_version, &CharacterList {
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
