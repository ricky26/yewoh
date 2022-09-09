use std::collections::HashMap;
use std::net::SocketAddr;

use bevy_ecs::prelude::*;
use glam::{UVec2, UVec3};
use log::{info, warn};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use yewoh::{Direction, EntityId, Notoriety};
use yewoh::protocol::{AnyPacket, BeginEnterWorld, ChangeSeason, CharacterList, ClientVersion, ClientVersionRequest, CreateCharacterEnhanced, EndEnterWorld, EntityFlags, ExtendedCommand, FeatureFlags, Move, MoveConfirm, Reader, SetTime, StartingCity, SupportedFeatures, UpsertEntityCharacter, UpsertEntityStats, UpsertLocalPlayer, Writer};
use crate::game_server::NewSessionAttempt;
use crate::lobby::{NewSession, NewSessionRequest, SessionAllocator};

pub struct NewConnection {
    pub address: SocketAddr,
    pub reader: Reader,
    pub writer: Writer,
}

pub enum WriterAction {
    Send(ClientVersion, AnyPacket),
}

#[derive(Debug, Clone, Component)]
pub struct NetClient {
    address: SocketAddr,
}

#[derive(Debug, Clone)]
pub struct ClientState {
    address: SocketAddr,
    client_version: ClientVersion,
    seed: u32,
    primary_entity: Option<Entity>,
    in_world: bool,
    tx: mpsc::UnboundedSender<WriterAction>,
}

impl ClientState {
    pub fn send_packet(&mut self, packet: AnyPacket) {
        log::debug!("OUT ({:?}): {:?}", self.address, packet);
        self.tx.send(WriterAction::Send(self.client_version, packet)).ok();
    }
}

pub struct PlayerServer {
    new_session_requests: mpsc::UnboundedReceiver<NewSessionRequest>,
    new_session_attempts: mpsc::UnboundedReceiver<NewSessionAttempt>,

    session_allocator: SessionAllocator,

    packet_rx: mpsc::UnboundedReceiver<(Entity, AnyPacket)>,
    packet_tx: mpsc::UnboundedSender<(Entity, AnyPacket)>,

    internal_close_tx: mpsc::UnboundedSender<Entity>,
    internal_close_rx: mpsc::UnboundedReceiver<Entity>,

    clients: HashMap<Entity, ClientState>,
}

impl PlayerServer {
    pub fn new(
        new_session_requests: mpsc::UnboundedReceiver<NewSessionRequest>,
        new_sessions: mpsc::UnboundedReceiver<NewSessionAttempt>,
    ) -> PlayerServer {
        let (internal_tx, packet_rx) = mpsc::unbounded_channel();
        let (internal_close_tx, internal_close_rx) = mpsc::unbounded_channel();

        Self {
            new_session_requests,
            new_session_attempts: new_sessions,
            session_allocator: SessionAllocator::new(),
            packet_rx,
            packet_tx: internal_tx,
            internal_close_tx,
            internal_close_rx,
            clients: HashMap::new(),
        }
    }

    pub fn send_packet(&mut self, entity: Entity, packet: AnyPacket) {
        if let Some(client) = self.clients.get_mut(&entity) {
            client.send_packet(packet);
        }
    }
}

pub fn accept_new_clients(runtime: Res<Handle>, mut server: ResMut<PlayerServer>,
    connections: Query<&NetClient>, mut commands: Commands) {
    while let Ok(new_session_request) = server.new_session_requests.try_recv() {
        server.session_allocator.allocate_session(new_session_request);
    }

    while let Ok(session_attempt) = server.new_session_attempts.try_recv() {
        let new_session = match server.session_allocator.start_session(session_attempt) {
            Ok(x) => x,
            Err(err) => {
                warn!("Failed to start session: {err}");
                continue;
            }
        };

        let NewSession {
            address,
            mut reader,
            mut writer,
            username,
            seed,
            client_version,
            ..
        } = new_session;
        let (tx, mut rx) = mpsc::unbounded_channel();

        info!("New game session from {} for {} (version {})", &address, &username, client_version);

        runtime.spawn(async move {
            while let Some(action) = rx.recv().await {
                match action {
                    WriterAction::Send(client_version, packet) => {
                        if let Err(err) = writer.send_any(client_version, &packet).await {
                            warn!("Error sending packet {err}");
                        }
                    }
                }
            }
        });

        let entity = commands.spawn()
            .insert(NetClient { address })
            .id();
        let client = ClientState {
            address,
            client_version,
            seed,
            primary_entity: None,
            in_world: false,
            tx,
        };
        server.clients.insert(entity, client);

        let internal_tx = server.packet_tx.clone();
        let internal_close = server.internal_close_tx.clone();
        let client = server.clients.get_mut(&entity).unwrap();

        runtime.spawn(async move {
            loop {
                match reader.recv(client_version).await {
                    Ok(packet) => {
                        if let Err(err) = internal_tx.send((entity, packet)) {
                            warn!("Error forwarding packet {err}");
                            break;
                        }
                    }
                    Err(err) => {
                        warn!("Error receiving packet {err}");
                        break;
                    }
                }
            }

            internal_close.send(entity).ok();
        });

        client.send_packet(ClientVersionRequest::default().into());
    }

    while let Ok(entity) = server.internal_close_rx.try_recv() {
        if let Ok(connection) = connections.get(entity) {
            info!("Connection from {} disconnected", connection.address);
        }

        commands.entity(entity).despawn();
    }
}

pub fn handle_packets(mut server: ResMut<PlayerServer>) {
    while let Ok((entity, packet)) = server.packet_rx.try_recv() {
        let client = match server.clients.get_mut(&entity) {
            Some(x) => x,
            _ => continue,
        };

        log::debug!("IN ({:?}): {:?}", client.address, packet);

        let entity_id = EntityId::from_u32(1337);
        let position = UVec3::new(500, 2000, 0);
        let direction = Direction::North;
        let body_type = 0x25e;
        let hue = 120;

        if let Some(_version_response) = packet.downcast::<ClientVersionRequest>() {
            if !client.in_world {
                client.in_world = true;
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
                }.into())
            }
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
            client.send_packet(UpsertEntityCharacter {
                id: entity_id,
                body_type,
                position,
                direction,
                hue,
                flags: EntityFlags::empty(),
                notoriety: Notoriety::Innocent,
                children: vec![],
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
        } else if let Some(request) = packet.downcast::<Move>() {
            client.send_packet(MoveConfirm {
                sequence: request.sequence,
                notoriety: Notoriety::Innocent,
            }.into());
        }
    }
}
