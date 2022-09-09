use std::collections::HashMap;
use std::net::SocketAddr;

use bevy_ecs::prelude::*;
use glam::UVec2;
use log::{info, warn};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use yewoh::protocol::{AnyPacket, AsciiTextMessageRequest, BeginEnterWorld, ChangeSeason, ClientVersion, ClientVersionRequest, CreateCharacterClassic, CreateCharacterEnhanced, DoubleClick, EndEnterWorld, EntityFlags, ExtendedCommand, FeatureFlags, Move, Reader, SetTime, SingleClick, SupportedFeatures, UnicodeTextMessageRequest, UpsertLocalPlayer, Writer};

use crate::game_server::NewSessionAttempt;
use crate::lobby::{NewSession, NewSessionRequest, SessionAllocator};
use crate::world::entity::{Character, MapPosition, NetEntity, NetEntityLookup, NetOwner, Stats};
use crate::world::events::{
    CharacterListEvent, ChatRequestEvent, CreateCharacterEvent, DoubleClickEvent, MoveEvent,
    NewPrimaryEntityEvent, ReceivedPacketEvent, SentPacketEvent, SingleClickEvent,
};

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
    pub primary_entity: Option<Entity>,
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
    in_world: bool,
    tx: mpsc::UnboundedSender<WriterAction>,
}

impl ClientState {
    pub fn send_packet(&self, packet: AnyPacket) {
        log::debug!("OUT ({:?}): {:?}", self.address, packet);
        self.tx.send(WriterAction::Send(self.client_version, packet)).ok();
    }
}

pub struct NetClients {
    new_session_requests: mpsc::UnboundedReceiver<NewSessionRequest>,
    new_session_attempts: mpsc::UnboundedReceiver<NewSessionAttempt>,

    session_allocator: SessionAllocator,

    packet_rx: mpsc::UnboundedReceiver<(Entity, AnyPacket)>,
    packet_tx: mpsc::UnboundedSender<(Entity, AnyPacket)>,

    internal_close_tx: mpsc::UnboundedSender<Entity>,
    internal_close_rx: mpsc::UnboundedReceiver<Entity>,

    clients: HashMap<Entity, ClientState>,
}

impl NetClients {
    pub fn new(
        new_session_requests: mpsc::UnboundedReceiver<NewSessionRequest>,
        new_sessions: mpsc::UnboundedReceiver<NewSessionAttempt>,
    ) -> NetClients {
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

    pub fn client(&self, entity: Entity) -> Option<&ClientState> {
        self.clients.get(&entity)
    }

    pub fn client_mut(&mut self, entity: Entity) -> Option<&mut ClientState> {
        self.clients.get_mut(&entity)
    }

    pub fn broadcast_packet(&self, packet: AnyPacket) {
        for client in self.clients.values() {
            client.send_packet(packet.clone());
        }
    }

    pub fn send_packet(&self, entity: Entity, packet: AnyPacket) {
        if let Some(client) = self.clients.get(&entity) {
            client.send_packet(packet);
        }
    }
}

pub fn accept_new_clients(runtime: Res<Handle>, mut server: ResMut<NetClients>,
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
            .insert(NetClient { address, primary_entity: None })
            .id();
        let client = ClientState {
            address,
            client_version,
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

pub fn handle_new_packets(
    mut server: ResMut<NetClients>,
    mut write_events: EventReader<SentPacketEvent>,
    mut read_events: EventWriter<ReceivedPacketEvent>,
) {
    while let Ok((connection, packet)) = server.packet_rx.try_recv() {
        let client = match server.client_mut(connection) {
            Some(x) => x,
            _ => continue,
        };

        log::debug!("IN ({:?}): {:?}", client.address, packet);
        read_events.send(ReceivedPacketEvent { connection, packet });
    }

    for SentPacketEvent { connection, packet } in write_events.iter() {
        server.send_packet(*connection, packet.clone());
    }
}

pub fn handle_login_packets(
    mut server: ResMut<NetClients>,
    mut events: EventReader<ReceivedPacketEvent>,
    mut character_list_events: EventWriter<CharacterListEvent>,
    mut character_creation_events: EventWriter<CreateCharacterEvent>,
) {
    for ReceivedPacketEvent { connection, packet } in events.iter() {
        let connection = *connection;
        let client = match server.client_mut(connection) {
            Some(x) => x,
            None => continue,
        };

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
        }
    }
}

pub fn handle_input_packets(
    lookup: Res<NetEntityLookup>,
    mut events: EventReader<ReceivedPacketEvent>,
    mut move_events: EventWriter<MoveEvent>,
    mut chat_events: EventWriter<ChatRequestEvent>,
    mut single_click_events: EventWriter<SingleClickEvent>,
    mut double_click_events: EventWriter<DoubleClickEvent>,
) {
    for ReceivedPacketEvent { connection, packet } in events.iter() {
        let connection = *connection;

        if let Some(request) = packet.downcast::<Move>().cloned() {
            move_events.send(MoveEvent { connection, request });
        } else if let Some(request) = packet.downcast::<SingleClick>() {
            single_click_events.send(SingleClickEvent {
                connection,
                target: lookup.net_to_ecs(request.target_id),
            });
        } else if let Some(request) = packet.downcast::<DoubleClick>() {
            double_click_events.send(DoubleClickEvent {
                connection,
                target: lookup.net_to_ecs(request.target_id),
            });
        } else if let Some(request) = packet.downcast::<AsciiTextMessageRequest>() {
            chat_events.send(ChatRequestEvent {
                connection,
                request: UnicodeTextMessageRequest {
                    kind: request.kind,
                    hue: request.hue,
                    font: request.font,
                    text: request.text.clone(),
                    ..Default::default()
                },
            });
        } else if let Some(request) = packet.downcast::<UnicodeTextMessageRequest>().cloned() {
            chat_events.send(ChatRequestEvent { connection, request });
        }
    }
}

pub fn apply_new_primary_entities(
    maps: Res<MapInfos>,
    server: Res<NetClients>,
    mut events: EventReader<NewPrimaryEntityEvent>,
    mut client_query: Query<&mut NetClient>,
    query: Query<(&NetEntity, &MapPosition, &Character)>,
    mut commands: Commands,
) {
    for event in events.iter() {
        let connection = event.connection;
        let client = match server.client(connection) {
            Some(v) => v,
            None => continue,
        };

        let mut component = match client_query.get_mut(connection) {
            Ok(x) => x,
            _ => continue,
        };

        if component.primary_entity == event.primary_entity {
            continue;
        }

        if let Some(old_primary) = component.primary_entity {
            commands.entity(old_primary).remove::<NetOwner>();
        }

        component.primary_entity = event.primary_entity;

        let primary_entity = match component.primary_entity {
            Some(x) => x,
            None => continue,
        };

        commands.entity(primary_entity).insert(NetOwner { connection });

        let (primary_net, map_position, character) = match query.get(primary_entity) {
            Ok(x) => x,
            Err(_) => {
                continue;
            }
        };

        let MapPosition { position, map_id, direction } = map_position.clone();
        let map = match maps.maps.get(&map_id) {
            Some(v) => v,
            None => continue,
        };

        let entity_id = primary_net.id;
        let body_type = character.body_type;

        client.send_packet(BeginEnterWorld {
            entity_id,
            body_type,
            position,
            direction,
            map_size: map.size,
        }.into());
        client.send_packet(ExtendedCommand::ChangeMap(map_id).into());
        client.send_packet(ChangeSeason { season: map.season, play_sound: true }.into());
        client.send_packet(EndEnterWorld.into());

        client.send_packet(SetTime {
            hour: 12,
            minute: 16,
            second: 31,
        }.into());
    }
}

pub fn send_player_updates(
    clients: Res<NetClients>,
    query: Query<
        (&NetOwner, &NetEntity, &Character, &MapPosition),
        Or<(Changed<Character>, Changed<MapPosition>)>,
    >,
) {
    for (owner, entity, character, position) in query.iter() {
        clients.send_packet(owner.connection, UpsertLocalPlayer {
            id: entity.id,
            body_type: character.body_type,
            server_id: 0,
            hue: character.hue,
            flags: EntityFlags::empty(),
            position: position.position,
            direction: position.direction,
        }.into());
    }
}
