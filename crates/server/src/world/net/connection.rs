use std::net::SocketAddr;
use std::sync::Arc;

use bevy_ecs::prelude::*;
use log::{info, warn};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use yewoh::protocol::{AnyPacket, AsciiTextMessageRequest, CharacterProfile, ClientVersion, ClientVersionRequest, CreateCharacterClassic, CreateCharacterEnhanced, DoubleClick, DropEntity, EntityTooltip, EquipEntity, FeatureFlags, GameServerLogin, Move, PickUpEntity, SelectCharacter, SingleClick, SupportedFeatures, UnicodeTextMessageRequest};
use yewoh::protocol::encryption::Encryption;

use crate::game_server::NewSessionAttempt;
use crate::lobby::{NewSessionRequest, SessionAllocator};
use crate::world::entity::Tooltip;
use crate::world::events::{CharacterListEvent, ChatRequestEvent, CreateCharacterEvent, DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, PickUpEvent, ProfileEvent, ReceivedPacketEvent, SelectCharacterEvent, SentPacketEvent, SingleClickEvent};
use crate::world::input::Targeting;
use crate::world::net::entity::NetEntityLookup;

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

#[derive(Debug, Clone, Component)]
pub struct User {
    pub username: String,
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

pub struct NetServer {
    encrypted: bool,

    new_session_requests: mpsc::UnboundedReceiver<NewSessionRequest>,
    new_session_attempts: mpsc::UnboundedReceiver<NewSessionAttempt>,

    login_attempts_tx: mpsc::UnboundedSender<(NewSessionAttempt, ClientVersion, GameServerLogin)>,
    login_attempts_rx: mpsc::UnboundedReceiver<(NewSessionAttempt, ClientVersion, GameServerLogin)>,

    session_allocator: SessionAllocator,

    received_packets_rx: mpsc::UnboundedReceiver<(Entity, AnyPacket)>,
    received_packets_tx: mpsc::UnboundedSender<(Entity, AnyPacket)>,

    closed_tx: mpsc::UnboundedSender<Entity>,
    closed_rx: mpsc::UnboundedReceiver<Entity>,
}

impl NetServer {
    pub fn new(
        encrypted: bool,
        new_session_requests: mpsc::UnboundedReceiver<NewSessionRequest>,
        new_sessions: mpsc::UnboundedReceiver<NewSessionAttempt>,
    ) -> NetServer {
        let (received_packets_tx, received_packets_rx) = mpsc::unbounded_channel();
        let (closed_tx, closed_rx) = mpsc::unbounded_channel();
        let (login_attempts_tx, login_attempts_rx) = mpsc::unbounded_channel();

        Self {
            encrypted,
            new_session_requests,
            new_session_attempts: new_sessions,
            session_allocator: SessionAllocator::new(),
            login_attempts_tx,
            login_attempts_rx,
            received_packets_rx,
            received_packets_tx,
            closed_tx,
            closed_rx,
        }
    }
}

pub fn broadcast<'a>(clients: impl Iterator<Item=&'a NetClient>, packet: Arc<AnyPacket>) {
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
        let client_version = match server.session_allocator.client_version_for_token(session_attempt.token) {
            Some(x) => x,
            None => {
                warn!("Session attempt for unknown token {}", session_attempt.token);
                continue;
            }
        };

        let NewSessionAttempt {
            address,
            mut reader,
            mut writer,
            token,
        } = session_attempt;

        if server.encrypted {
            let encryption = Encryption::new(client_version, token, false);
            reader.set_encryption(Some(encryption.clone()));
            writer.set_encryption(Some(encryption));
        }

        let attempt_tx = server.login_attempts_tx.clone();
        runtime.spawn(async move {
            let packet = match reader.recv(ClientVersion::default()).await {
                Ok(packet) => packet,
                Err(err) => {
                    warn!("From ({address}): whilst reading first packet: {err}");
                    return;
                }
            };

            let login = match packet.into_downcast::<GameServerLogin>().ok() {
                Some(packet) => packet,
                None => {
                    warn!("From ({address}): expected login as first game server connection message");
                    return;
                }
            };

            if login.token != token {
                warn!("From ({address}): expected initial token & login token to match");
                return;
            }

            attempt_tx.send((NewSessionAttempt {
                address,
                reader,
                writer,
                token,
            }, client_version, login)).ok();
        });
    }

    while let Ok((session_attempt, client_version, login)) = server.login_attempts_rx.try_recv() {
        let NewSessionAttempt {
            address,
            mut reader,
            mut writer,
            token,
        } = session_attempt;
        let new_session = match server.session_allocator.start_session(token, login) {
            Ok(x) => x,
            Err(err) => {
                warn!("Failed to start session: {err}");
                continue;
            }
        };

        let username = new_session.username;
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
            .insert(User { username })
            .insert(Targeting::default())
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
        read_events.send(ReceivedPacketEvent { client_entity: connection, packet });
    }

    for SentPacketEvent { client_entity: connection, packet } in write_events.iter() {
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
    mut select_character_events: EventWriter<SelectCharacterEvent>,
    mut commands: Commands,
) {
    for ReceivedPacketEvent { client_entity: connection, packet } in events.iter() {
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
                    client_entity: connection,
                });
            }
        } else if let Some(create_character) = packet.downcast::<CreateCharacterClassic>() {
            character_creation_events.send(CreateCharacterEvent {
                client_entity: connection,
                request: create_character.0.clone(),
            });
        } else if let Some(create_character) = packet.downcast::<CreateCharacterEnhanced>() {
            character_creation_events.send(CreateCharacterEvent {
                client_entity: connection,
                request: create_character.0.clone(),
            });
        } else if let Some(select_character) = packet.downcast::<SelectCharacter>() {
            select_character_events.send(SelectCharacterEvent {
                client_entity: connection,
                request: select_character.clone(),
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
    mut pick_up_events: EventWriter<PickUpEvent>,
    mut drop_events: EventWriter<DropEvent>,
    mut equip_events: EventWriter<EquipEvent>,
    mut profile_events: EventWriter<ProfileEvent>,
) {
    for ReceivedPacketEvent { client_entity: connection, packet } in events.iter() {
        let connection = *connection;

        if let Some(request) = packet.downcast::<Move>().cloned() {
            move_events.send(MoveEvent { client_entity: connection, request });
        } else if let Some(request) = packet.downcast::<SingleClick>() {
            single_click_events.send(SingleClickEvent {
                client_entity: connection,
                target: lookup.net_to_ecs(request.target_id),
            });
        } else if let Some(request) = packet.downcast::<DoubleClick>() {
            double_click_events.send(DoubleClickEvent {
                client_entity: connection,
                target: lookup.net_to_ecs(request.target_id),
            });
        } else if let Some(request) = packet.downcast::<PickUpEntity>() {
            if let Some(target) = lookup.net_to_ecs(request.target_id) {
                pick_up_events.send(PickUpEvent {
                    client_entity: connection,
                    target,
                });
            }
        } else if let Some(request) = packet.downcast::<DropEntity>() {
            if let Some(target) = lookup.net_to_ecs(request.target_id) {
                drop_events.send(DropEvent {
                    client_entity: connection,
                    target: target,
                    position: request.position,
                    grid_index: request.grid_index,
                    dropped_on: request.container_id.and_then(|id| lookup.net_to_ecs(id)),
                });
            }
        } else if let Some(request) = packet.downcast::<EquipEntity>() {
            if let Some((target, character)) = lookup.net_to_ecs(request.target_id)
                .zip(lookup.net_to_ecs(request.character_id)) {
                equip_events.send(EquipEvent {
                    client_entity: connection,
                    target,
                    character,
                    slot: request.slot,
                });
            }
        } else if let Some(request) = packet.downcast::<AsciiTextMessageRequest>() {
            chat_events.send(ChatRequestEvent {
                client_entity: connection,
                request: UnicodeTextMessageRequest {
                    kind: request.kind,
                    hue: request.hue,
                    font: request.font,
                    text: request.text.clone(),
                    ..Default::default()
                },
            });
        } else if let Some(request) = packet.downcast::<UnicodeTextMessageRequest>().cloned() {
            chat_events.send(ChatRequestEvent { client_entity: connection, request });
        } else if let Some(request) = packet.downcast::<CharacterProfile>().cloned() {
            match request {
                CharacterProfile::Request(request) => {
                    if let Some(target) = lookup.net_to_ecs(request.target_id) {
                        profile_events.send(ProfileEvent {
                            client_entity: connection,
                            target,
                            new_profile: request.new_profile.clone(),
                        });
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

pub fn send_tooltips(
    lookup: Res<NetEntityLookup>,
    clients: Query<&NetClient>,
    tooltips: Query<&Tooltip>,
    mut events: EventReader<ReceivedPacketEvent>,
) {
    for ReceivedPacketEvent { client_entity: connection, packet } in events.iter() {
        let connection = *connection;
        let client = match clients.get(connection) {
            Ok(x) => x,
            _ => continue,
        };

        let request = match packet.downcast::<EntityTooltip>() {
            Some(x) => x,
            _ => continue,
        };

        match request {
            EntityTooltip::Request(ids) => {
                for id in ids.iter().copied() {
                    let entries = lookup.net_to_ecs(id)
                        .and_then(|e| tooltips.get(e).ok())
                        .map_or(Vec::new(), |t| t.entries.clone());
                    client.send_packet(EntityTooltip::Response { id, entries }.into());
                }
            }
            _ => {}
        }
    }
}
