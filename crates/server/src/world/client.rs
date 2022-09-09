use std::collections::HashMap;
use std::net::SocketAddr;

use bevy_ecs::prelude::*;
use glam::UVec2;
use log::{info, warn};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use yewoh::protocol::{AnyPacket, BeginEnterWorld, ChangeSeason, ClientVersion, ClientVersionRequest, CreateCharacterClassic, CreateCharacterEnhanced, EndEnterWorld, EntityFlags, ExtendedCommand, FeatureFlags, Move, Reader, SetTime, SupportedFeatures, UpsertEntityCharacter, UpsertLocalPlayer, Writer};

use crate::game_server::NewSessionAttempt;
use crate::lobby::{NewSession, NewSessionRequest, SessionAllocator};
use crate::world::entity::{EntityVisual, EntityVisualKind, HasNotoriety, MapPosition, NetEntity, Stats};
use crate::world::events::{CharacterListEvent, CreateCharacterEvent, MoveEvent, NewPrimaryEntityEvent};

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
    pub address: SocketAddr,
}

#[derive(Debug, Clone, Component)]
pub struct User {
    pub username: String,
}

#[derive(Debug, Clone)]
pub struct MapInfo {
    pub size: UVec2,
    pub season: u8,
}

#[derive(Debug, Clone, Default)]
pub struct MapInfos {
    pub maps: HashMap<u8, MapInfo>,
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

    pub fn client_mut(&mut self, entity: Entity) -> Option<&mut ClientState> {
        self.clients.get_mut(&entity)
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
        let client = server.client_mut(entity).unwrap();

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

pub fn handle_packets(
    mut server: ResMut<PlayerServer>,
    mut character_list_events: EventWriter<CharacterListEvent>,
    mut character_creation_events: EventWriter<CreateCharacterEvent>,
    mut move_events: EventWriter<MoveEvent>,
) {
    while let Ok((connection, packet)) = server.packet_rx.try_recv() {
        let client = match server.client_mut(connection) {
            Some(x) => x,
            _ => continue,
        };

        log::debug!("IN ({:?}): {:?}", client.address, packet);

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
                character_list_events.send(CharacterListEvent {
                    connection,
                });
            }
        } else if let Some(create_character) = packet.downcast::<CreateCharacterClassic>() {
            character_creation_events.send(CreateCharacterEvent {
                connection,
                request: create_character.0.clone(),
            });
        } else if let Some(create_character) = packet.downcast::<CreateCharacterEnhanced>() {
            character_creation_events.send(CreateCharacterEvent {
                connection,
                request: create_character.0.clone(),
            });
        } else if let Some(request) = packet.downcast::<Move>().cloned() {
            move_events.send(MoveEvent {
                connection,
                primary_entity: client.primary_entity,
                request,
            });
        }
    }
}

pub fn apply_new_primary_entities(
    maps: Res<MapInfos>,
    mut server: ResMut<PlayerServer>,
    mut events: EventReader<NewPrimaryEntityEvent>,
    query: Query<(&NetEntity, &MapPosition, &EntityVisual, &Stats, &HasNotoriety)>,
) {
    for event in events.iter() {
        log::info!("New Pri");
        let client = match server.client_mut(event.connection) {
            Some(v) => v,
            None => continue,
        };

        log::info!("New Pri2 {:?}", event.primary_entity);
        let (primary_net, map_position, visual, stats, notoriety) = match query.get(event.primary_entity) {
            Ok(x) => x,
            Err(err) => {
                log::info!("ax {err}");
                continue;
            }
        };
        log::info!("New Pri3");

        let notoriety = **notoriety;
        let MapPosition { position, map_id, direction } = map_position.clone();
        let map = match maps.maps.get(&map_id) {
            Some(v) => v,
            None => continue,
        };

        log::info!("New Pri5");
        client.primary_entity = Some(event.primary_entity);
        let entity_id = primary_net.id;

        let hue = visual.hue;
        let body_type = match visual.kind {
            EntityVisualKind::Body(body_type) => body_type,
            _ => 0,
        };

        client.send_packet(BeginEnterWorld {
            entity_id,
            body_type,
            position,
            direction,
            map_size: map.size,
        }.into());
        client.send_packet(ExtendedCommand::ChangeMap(map_id).into());
        client.send_packet(ChangeSeason { season: map.season, play_sound: true }.into());

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
            notoriety,
            children: vec![],
        }.into());
        client.send_packet(stats.upsert(entity_id, true).into());
        client.send_packet(EndEnterWorld.into());

        client.send_packet(SetTime {
            hour: 12,
            minute: 16,
            second: 31,
        }.into());
    }
}
