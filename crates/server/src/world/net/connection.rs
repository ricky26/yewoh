use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use bevy_ecs::prelude::*;
use glam::UVec2;
use log::{info, warn};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use yewoh::protocol::{
    AnyPacket, AsciiTextMessageRequest, ClientVersion, ClientVersionRequest, CreateCharacterClassic,
    CreateCharacterEnhanced, DoubleClick, EntityFlags, FeatureFlags, Move, SingleClick, SupportedFeatures,
    UnicodeTextMessageRequest, UpsertLocalPlayer,
};

use crate::game_server::NewSessionAttempt;
use crate::lobby::{NewSession, NewSessionRequest, SessionAllocator};
use crate::world::entity::{Character, MapPosition};
use crate::world::events::{
    CharacterListEvent, ChatRequestEvent, CreateCharacterEvent, DoubleClickEvent, MoveEvent,
    ReceivedPacketEvent, SentPacketEvent, SingleClickEvent,
};
use crate::world::net::entity::{NetEntity, NetEntityLookup};
use crate::world::net::owner::NetOwner;

pub enum WriterAction {
    Send(ClientVersion, AnyPacket),
    SendArc(ClientVersion, Arc<AnyPacket>),
}

#[derive(Debug, Clone, Component)]
pub struct NetClient {
    address: SocketAddr,
    client_version: ClientVersion,
    tx: mpsc::UnboundedSender<WriterAction>,
}

impl NetClient {
    pub fn address(&self) -> SocketAddr { self.address }

    pub fn client_version(&self) -> ClientVersion { self.client_version }

    pub fn send_packet(&self, packet: AnyPacket) {
        log::debug!("OUT ({:?}): {:?}", self.address, packet);
        self.tx.send(WriterAction::Send(self.client_version, packet)).ok();
    }

    pub fn send_packet_arc(&self, packet: impl Into<Arc<AnyPacket>>) {
        let packet = packet.into();
        log::debug!("OUT ({:?}): {:?}", self.address, packet);
        self.tx.send(WriterAction::SendArc(self.client_version, packet)).ok();
    }
}

#[derive(Debug, Clone, Component)]
pub struct NetInWorld;

#[derive(Debug, Clone)]
pub struct MapInfo {
    pub size: UVec2,
    pub season: u8,
}

#[derive(Debug, Clone, Default)]
pub struct MapInfos {
    pub maps: HashMap<u8, MapInfo>,
}

pub struct NetServer {
    new_session_requests: mpsc::UnboundedReceiver<NewSessionRequest>,
    new_session_attempts: mpsc::UnboundedReceiver<NewSessionAttempt>,

    session_allocator: SessionAllocator,

    received_packets_rx: mpsc::UnboundedReceiver<(Entity, AnyPacket)>,
    received_packets_tx: mpsc::UnboundedSender<(Entity, AnyPacket)>,

    closed_tx: mpsc::UnboundedSender<Entity>,
    closed_rx: mpsc::UnboundedReceiver<Entity>,
}

impl NetServer {
    pub fn new(
        new_session_requests: mpsc::UnboundedReceiver<NewSessionRequest>,
        new_sessions: mpsc::UnboundedReceiver<NewSessionAttempt>,
    ) -> NetServer {
        let (internal_tx, packet_rx) = mpsc::unbounded_channel();
        let (internal_close_tx, internal_close_rx) = mpsc::unbounded_channel();

        Self {
            new_session_requests,
            new_session_attempts: new_sessions,
            session_allocator: SessionAllocator::new(),
            received_packets_rx: packet_rx,
            received_packets_tx: internal_tx,
            closed_tx: internal_close_tx,
            closed_rx: internal_close_rx,
        }
    }
}

pub fn broadcast<'a>(clients: impl Iterator<Item = &'a NetClient>, packet: Arc<AnyPacket>) {
    for client in clients {
        client.send_packet_arc(packet.clone());
    }
}

pub fn accept_new_clients(runtime: Res<Handle>, mut server: ResMut<NetServer>,
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
                    WriterAction::SendArc(client_version, packet) => {
                        if let Err(err) = writer.send_any(client_version, &packet).await {
                            warn!("Error sending packet {err}");
                        }
                    }
                }
            }
        });

        let client = NetClient { address, client_version, tx };
        let entity = commands.spawn()
            .insert(client.clone())
            .id();

        let internal_tx = server.received_packets_tx.clone();
        let internal_close = server.closed_tx.clone();

        runtime.spawn(async move {
            loop {
                match reader.recv(client_version).await {
                    Ok(packet) => {
                        log::debug!("IN ({:?}): {:?}", address, packet);
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

    while let Ok(entity) = server.closed_rx.try_recv() {
        if let Ok(connection) = connections.get(entity) {
            info!("Connection from {} disconnected", connection.address);
        }

        commands.entity(entity).despawn();
    }
}

pub fn handle_new_packets(
    mut server: ResMut<NetServer>,
    clients: Query<&NetClient>,
    mut write_events: EventReader<SentPacketEvent>,
    mut read_events: EventWriter<ReceivedPacketEvent>,
) {
    while let Ok((connection, packet)) = server.received_packets_rx.try_recv() {
        read_events.send(ReceivedPacketEvent { client: connection, packet });
    }

    for SentPacketEvent { client: connection, packet } in write_events.iter() {
        match connection {
            Some(entity) => {
                if let Ok(client) = clients.get(*entity) {
                    client.send_packet_arc(packet.clone());
                }
            }
            None => broadcast(clients.iter(), packet.clone()),
        }
    }
}

pub fn handle_login_packets(
    clients: Query<&NetClient>,
    in_world: Query<&NetInWorld>,
    mut events: EventReader<ReceivedPacketEvent>,
    mut character_list_events: EventWriter<CharacterListEvent>,
    mut character_creation_events: EventWriter<CreateCharacterEvent>,
    mut commands: Commands,
) {
    for ReceivedPacketEvent { client: connection, packet } in events.iter() {
        let connection = *connection;
        let client = match clients.get(connection) {
            Ok(x) => x,
            _ => continue,
        };

        if let Some(_version_response) = packet.downcast::<ClientVersionRequest>() {
            if !in_world.contains(connection) {
                commands.entity(connection).insert(NetInWorld);

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
                    client: connection,
                });
            }
        } else if let Some(create_character) = packet.downcast::<CreateCharacterClassic>() {
            character_creation_events.send(CreateCharacterEvent {
                client: connection,
                request: create_character.0.clone(),
            });
        } else if let Some(create_character) = packet.downcast::<CreateCharacterEnhanced>() {
            character_creation_events.send(CreateCharacterEvent {
                client: connection,
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
    for ReceivedPacketEvent { client: connection, packet } in events.iter() {
        let connection = *connection;

        if let Some(request) = packet.downcast::<Move>().cloned() {
            move_events.send(MoveEvent { client: connection, request });
        } else if let Some(request) = packet.downcast::<SingleClick>() {
            single_click_events.send(SingleClickEvent {
                client: connection,
                target: lookup.net_to_ecs(request.target_id),
            });
        } else if let Some(request) = packet.downcast::<DoubleClick>() {
            double_click_events.send(DoubleClickEvent {
                client: connection,
                target: lookup.net_to_ecs(request.target_id),
            });
        } else if let Some(request) = packet.downcast::<AsciiTextMessageRequest>() {
            chat_events.send(ChatRequestEvent {
                client: connection,
                request: UnicodeTextMessageRequest {
                    kind: request.kind,
                    hue: request.hue,
                    font: request.font,
                    text: request.text.clone(),
                    ..Default::default()
                },
            });
        } else if let Some(request) = packet.downcast::<UnicodeTextMessageRequest>().cloned() {
            chat_events.send(ChatRequestEvent { client: connection, request });
        }
    }
}

pub fn send_player_updates(
    clients: Query<&NetClient>,
    query: Query<
        (&NetOwner, &NetEntity, &Character, &MapPosition),
        Or<(Changed<Character>, Changed<MapPosition>)>,
    >,
) {
    for (owner, entity, character, position) in query.iter() {
        if let Ok(client) = clients.get(owner.client) {
            client.send_packet(UpsertLocalPlayer {
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
}
