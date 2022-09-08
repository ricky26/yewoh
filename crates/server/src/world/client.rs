use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};

use bevy_ecs::prelude::*;
use glam::UVec3;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use yewoh::protocol::{AccountLogin, AnyPacket, CharacterList, ClientVersion, ClientVersionRequest, CreateCharacterEnhanced, EnterWorld, FeatureFlags, GameServer, GameServerLogin, Packet, Reader, Ready, Seed, SelectGameServer, ServerList, StartingCity, SupportedFeatures, SwitchServer, Writer};

pub struct NewConnection {
    pub address: SocketAddr,
    pub reader: Reader,
    pub writer: Writer,
}

pub enum WriterAction {
    Send(ClientVersion, AnyPacket),
    EnableCompression,
}

#[derive(Clone, Component)]
pub struct NetClient {
    address: SocketAddr,
    client_version: ClientVersion,
    seed: u32,
    tx: mpsc::UnboundedSender<WriterAction>,
}

impl NetClient {
    pub fn enable_compression(&mut self) {
        self.tx.send(WriterAction::EnableCompression).ok();
    }

    pub fn send_packet(&mut self, packet: AnyPacket) {
        self.tx.send(WriterAction::Send(self.client_version, packet)).ok();
    }
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
    connections: Query<&NetClient>, mut commands: Commands) {
    while let Ok(new_connection) = server.new_connections.try_recv() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let mut writer = new_connection.writer;
        runtime.spawn(async move {
            while let Some(action) = rx.recv().await {
                match action {
                    WriterAction::Send(client_version, packet) => {
                        if let Err(err) = writer.send_any(client_version, &packet).await {
                            log::warn!("Error sending packet {err}");
                        }
                    }
                    WriterAction::EnableCompression => writer.enable_compression(),
                }
            }
        });

        log::info!("New connection from {}", new_connection.address);

        let entity = commands.spawn()
            .insert(NetClient {
                address: new_connection.address,
                seed: 0,
                client_version: ClientVersion::default(),
                tx,
            })
            .id();

        let mut reader = new_connection.reader;
        let mut internal_tx = server.internal_tx.clone();
        let mut internal_close = server.internal_close_tx.clone();
        runtime.spawn(async move {
            let mut client_version = ClientVersion::new(8, 0, 0, 0);

            while let Ok(packet) = reader.receive(client_version).await {
                if let Some(seed) = packet.downcast::<Seed>() {
                    client_version = seed.client_version;
                }

                if let Err(err) = internal_tx.send((entity, packet)) {
                    log::warn!("Error forwarding packet {err}");
                    return;
                }
            }

            internal_close.send(entity).ok();
        });
    }

    while let Ok(entity) = server.internal_close_rx.try_recv() {
        if let Ok(connection) = connections.get(entity) {
            log::info!("Connection from {} disconnected", connection.address);
        }

        commands.entity(entity).despawn();
    }
}

pub fn handle_packets(mut server: ResMut<PlayerServer>, mut clients: Query<(&mut NetClient)>) {
    while let Ok((entity, packet)) = server.packet_rx.try_recv() {
        let mut client = match clients.get_mut(entity) {
            Ok(x) => x,
            _ => continue,
        };

        log::debug!("Got packet {:?}", packet);

        if let Some(seed) = packet.downcast::<Seed>() {
            client.seed = seed.seed;
            client.client_version = seed.client_version;
        } else if let Some(account_login) = packet.downcast::<AccountLogin>() {
            log::info!("New login attempt for {}", &account_login.username);
            client.send_packet(ServerList {
                system_info_flags: 0x5d,
                game_servers: vec![
                    GameServer {
                        server_name: "My Server".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }.into());
        } else if let Some(game_server) = packet.downcast::<SelectGameServer>() {
            // TODO: pass this across using token
            client.send_packet(SwitchServer {
                ip: Ipv4Addr::LOCALHOST.into(),
                port: 2593,
                token: 7,
            }.into());
        } else if let Some(game_login) = packet.downcast::<GameServerLogin>() {
            log::debug!("Game login {:?}", game_login);
            client.client_version = ClientVersion::new(8, 0, 0, 0);
            client.enable_compression();
            client.send_packet(SupportedFeatures {
                feature_flags: FeatureFlags::T2A
                    | FeatureFlags::UOR
                    | FeatureFlags::LBR
                    | FeatureFlags::AOS
                    | FeatureFlags::SE
                    | FeatureFlags::ML
                    | FeatureFlags::NINTH_AGE
                    | FeatureFlags::LIVE_ACCOUNT
                    | FeatureFlags::SA
                    | FeatureFlags::HS
                    | FeatureFlags::GOTHIC
                    | FeatureFlags::RUSTIC
                    | FeatureFlags::JUNGLE
                    | FeatureFlags::SHADOWGUARD
                    | FeatureFlags::TOL
                    | FeatureFlags::EJ,
            }.into());
            client.send_packet(CharacterList {
                characters: vec![None; 5],
                cities: vec![StartingCity {
                    index: 0,
                    city: "My City".into(),
                    building: "My Building".into(),
                    location: UVec3::new(0, 1, 2),
                    map_id: 0,
                    description_id: 0,
                }],
            }.into());
        } else if let Some(create_character) = packet.downcast::<CreateCharacterEnhanced>() {
            client.send_packet(ClientVersionRequest::default().into());
        } else if let Some(version_request) = packet.downcast::<ClientVersionRequest>() {
            client.send_packet(EnterWorld {
                mobile_id: 1234,
                body: 12,
                position: Default::default(),
                direction: 2,
                map_width: 1000,
                map_height: 1000
            }.into());
            client.send_packet(Ready.into());
        }
    }
}
