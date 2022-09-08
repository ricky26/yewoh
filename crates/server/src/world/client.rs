use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;

use bevy_ecs::prelude::*;
use glam::{UVec2, UVec3};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use yewoh::{Direction, EntityId, Notoriety};

use yewoh::protocol::{AccountLogin, AnyPacket, CharacterList, ClientVersion, ClientVersionRequest, CreateCharacterEnhanced, BeginEnterWorld, FeatureFlags, GameServer, GameServerLogin, Reader, EndEnterWorld, Seed, SelectGameServer, ServerList, StartingCity, SupportedFeatures, SwitchServer, Writer, ExtendedCommand, ChangeSeason, SetTime, ExtendedClientVersion, UpsertLocalPlayer, EntityFlags, UpsertEntityCharacter, UpsertEntityStats, Move, MoveConfirm};

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
        log::debug!("OUT: {:?}", packet);
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
        let internal_tx = server.internal_tx.clone();
        let internal_close = server.internal_close_tx.clone();
        runtime.spawn(async move {
            let mut client_version = ClientVersion::new(0, 0, 0, 0);

            loop {
                match reader.receive(client_version).await {
                    Ok(packet) => {
                        if let Some(seed) = packet.downcast::<Seed>() {
                            client_version = seed.client_version;
                        }

                        if let Err(err) = internal_tx.send((entity, packet)) {
                            log::warn!("Error forwarding packet {err}");
                            break;
                        }
                    }
                    Err(err) => {
                        log::warn!("Error receiving packet {err}");
                        break;
                    }
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

pub fn handle_packets(mut server: ResMut<PlayerServer>, mut clients: Query<&mut NetClient>) {
    while let Ok((entity, packet)) = server.packet_rx.try_recv() {
        let mut client = match clients.get_mut(entity) {
            Ok(x) => x,
            _ => continue,
        };

        log::debug!("IN: {:?}", packet);

        let entity_id = EntityId::from_u32(1337);
        let position = UVec3::new(500, 2000, 0);
        let direction = Direction::North;
        let body_type = 0x25e;
        let hue = 120;

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
        } else if let Some(_game_server) = packet.downcast::<SelectGameServer>() {
            // TODO: pass this across using token
            client.send_packet(SwitchServer {
                ip: Ipv4Addr::LOCALHOST.into(),
                port: 2593,
                token: 7,
            }.into());
        } else if let Some(_game_login) = packet.downcast::<GameServerLogin>() {
            client.enable_compression();

            // HACK: assume new client for now
            client.client_version = ClientVersion::new(8, 0, 0, 0);
            client.send_packet(ClientVersionRequest::default().into());

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
                    position: UVec3::new(0, 1, 2),
                    map_id: 0,
                    description_id: 0,
                }],
            }.into());
        } else if let Some(_create_character) = packet.downcast::<CreateCharacterEnhanced>() {
            client.send_packet(BeginEnterWorld {
                entity_id,
                body_type,
                position,
                direction,
                map_size: UVec2::new(5000, 5000),
            }.into());
            client.send_packet(ExtendedCommand::ChangeMap(0).into());
            client.send_packet(ChangeSeason { season: 0, play_sound: true }.into());

            client.send_packet(UpsertLocalPlayer {
                id: entity_id,
                body_type,
                server_id: 0,
                hue,
                flags: EntityFlags::empty(),
                position,
                direction,
            }.into());
            client.send_packet(UpsertEntityCharacter{
                id: entity_id,
                body_type,
                position,
                direction,
                hue,
                flags: EntityFlags::empty(),
                notoriety: Notoriety::Innocent,
                children: vec![]
            }.into());
            client.send_packet(UpsertEntityStats {
                id: entity_id,
                name: "CoolGuy".into(),
                max_info_level: 100,
                race_and_gender: 1,
                hp: 100,
                max_hp: 120,
                ..Default::default()
            }.into());

            client.send_packet(EndEnterWorld.into());

            client.send_packet(SetTime {
                hour: 12,
                minute: 16,
                second: 31,
            }.into());
        } else if let Some(version_request) = packet.downcast::<ClientVersionRequest>() {
            match ExtendedClientVersion::from_str(&version_request.version) {
                Ok(client_version) => client.client_version = client_version.client_version,
                Err(err) => {
                    log::warn!("Unable to parse client version '{}': {err}", &version_request.version);
                }
            }
        } else if let Some(request) = packet.downcast::<Move>() {
            client.send_packet(MoveConfirm {
                sequence: request.sequence,
                notoriety: Notoriety::Innocent,
            }.into());
        }
    }
}
