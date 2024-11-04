use std::net::SocketAddr;
use std::sync::Arc;

use bevy::prelude::*;
use tokio::sync::mpsc;
use tracing::{info, trace, warn};
use yewoh::protocol::encryption::Encryption;
use yewoh::protocol::{AnyPacket, CharacterProfile, ClientVersion, ClientVersionRequest, EntityRequestKind, EntityTooltip, EntityTooltipLine, FeatureFlags, GameServerLogin, IntoAnyPacket, SupportedFeatures, UnicodeTextMessageRequest};

use crate::async_runtime::AsyncRuntime;
use crate::game_server::NewSessionAttempt;
use crate::lobby::{NewSessionRequest, SessionAllocator};
use crate::world::account::{CharacterListEvent, CreateCharacterEvent, DeleteCharacterEvent, SelectCharacterEvent, SentCharacterList, User};
use crate::world::characters::{ProfileEvent, RequestSkillsEvent};
use crate::world::chat::ChatRequestEvent;
use crate::world::entity::{TooltipRequest, TooltipRequests};
use crate::world::input::{DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, PickUpEvent, SingleClickEvent, Targeting};
use crate::world::net_id::{NetEntityLookup, NetId};
use crate::world::view::View;
use crate::world::ServerSet;

pub enum WriterAction {
    Send(ClientVersion, AnyPacket),
    SendArc(ClientVersion, Arc<AnyPacket>),
}

#[derive(Debug, Clone, Component, Reflect)]
pub struct Possessing {
    pub entity: Entity,
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct OwningClient {
    pub client_entity: Entity,
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

    pub fn send_packet(&self, packet: impl IntoAnyPacket) {
        let action = match packet.into_any_maybe_arc() {
            Ok(p) => WriterAction::Send(self.client_version, p),
            Err(p) => WriterAction::SendArc(self.client_version, p),
        };
        self.tx.send(action).ok();
    }
}

#[derive(Debug, Event)]
pub struct ReceivedPacketEvent {
    pub client_entity: Entity,
    pub packet: AnyPacket,
}

#[derive(Resource)]
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

pub fn broadcast<'a>(clients: impl Iterator<Item=&'a NetClient>, packet: impl IntoAnyPacket) {
    let packet = packet.into_any_arc();
    for client in clients {
        client.send_packet(packet.clone());
    }
}

pub fn accept_new_clients(
    runtime: Res<AsyncRuntime>, mut server: ResMut<NetServer>,
    connections: Query<&NetClient>, mut commands: Commands,
) {
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
                Ok(Some(packet)) => packet,
                Ok(None) => return,
                Err(err) => {
                    warn!("From ({address}): whilst reading first packet: {err}");
                    return;
                }
            };

            let login = match packet {
                AnyPacket::GameServerLogin(packet) => packet,
                _ => {
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
                        trace!("OUT ({address:?}): {packet:?}");
                        if let Err(err) = writer.send(client_version, &packet).await {
                            warn!("Error sending packet {err}");
                        }
                    }
                    WriterAction::SendArc(client_version, packet) => {
                        trace!("OUT ({address:?}): {packet:?}");
                        if let Err(err) = writer.send(client_version, &*packet).await {
                            warn!("Error sending packet {err}");
                        }
                    }
                }
            }
        });

        let client = NetClient { address, client_version, tx };
        let entity = commands
            .spawn((
                client.clone(),
                User { username },
                Targeting::default(),
                View::default(),
            ))
            .id();

        let internal_tx = server.received_packets_tx.clone();
        let internal_close = server.closed_tx.clone();

        runtime.spawn(async move {
            loop {
                match reader.recv(client_version).await {
                    Ok(Some(packet)) => {
                        trace!("IN ({address:?}): {packet:?}");
                        if let Err(err) = internal_tx.send((entity, packet)) {
                            warn!("Error forwarding packet {err}");
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        warn!("Error receiving packet {err}");
                        break;
                    }
                }
            }

            internal_close.send(entity).ok();
        });

        client.send_packet(ClientVersionRequest::default());
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
    mut read_events: EventWriter<ReceivedPacketEvent>,
) {
    while let Ok((connection, packet)) = server.received_packets_rx.try_recv() {
        read_events.send(ReceivedPacketEvent { client_entity: connection, packet });
    }
}

pub fn handle_login_packets(
    clients: Query<(&NetClient, Option<&SentCharacterList>)>,
    mut events: EventReader<ReceivedPacketEvent>,
    mut character_list_events: EventWriter<CharacterListEvent>,
    mut character_creation_events: EventWriter<CreateCharacterEvent>,
    mut select_character_events: EventWriter<SelectCharacterEvent>,
    mut delete_character_events: EventWriter<DeleteCharacterEvent>,
    mut commands: Commands,
) {
    for ReceivedPacketEvent { client_entity: connection, packet } in events.read() {
        let connection = *connection;
        let (client, sent_character_list) = match clients.get(connection) {
            Ok(x) => x,
            _ => continue,
        };

        match packet {
            AnyPacket::ClientVersionRequest(_) => {
                if sent_character_list.is_none() {
                    commands.entity(connection).insert(SentCharacterList);

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
                    });
                    character_list_events.send(CharacterListEvent {
                        client_entity: connection,
                    });
                }
            }
            AnyPacket::CreateCharacterClassic(create_character) => {
                character_creation_events.send(CreateCharacterEvent {
                    client_entity: connection,
                    request: create_character.0.clone(),
                });
            }
            AnyPacket::CreateCharacterEnhanced(create_character) => {
                character_creation_events.send(CreateCharacterEvent {
                    client_entity: connection,
                    request: create_character.0.clone(),
                });
            }
            AnyPacket::SelectCharacter(select_character) => {
                select_character_events.send(SelectCharacterEvent {
                    client_entity: connection,
                    request: select_character.clone(),
                });
            }
            AnyPacket::DeleteCharacter(delete_character) => {
                delete_character_events.send(DeleteCharacterEvent {
                    client_entity: connection,
                    request: delete_character.clone(),
                });
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
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
    mut skills_events: EventWriter<RequestSkillsEvent>,
) {
    for ReceivedPacketEvent { client_entity: connection, packet } in events.read() {
        let client_entity = *connection;

        match packet {
            AnyPacket::Move(request) => {
                move_events.send(MoveEvent { client_entity, request: request.clone() });
            }
            AnyPacket::SingleClick(request) => {
                single_click_events.send(SingleClickEvent {
                    client_entity,
                    target: lookup.net_to_ecs(request.target_id),
                });
            }
            AnyPacket::DoubleClick(request) => {
                double_click_events.send(DoubleClickEvent {
                    client_entity,
                    target: lookup.net_to_ecs(request.target_id),
                });
            }
            AnyPacket::PickUpEntity(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    pick_up_events.send(PickUpEvent {
                        client_entity,
                        target,
                    });
                }
            }
            AnyPacket::DropEntity(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    drop_events.send(DropEvent {
                        client_entity,
                        target,
                        position: request.position,
                        grid_index: request.grid_index,
                        dropped_on: request.container_id.and_then(|id| lookup.net_to_ecs(id)),
                    });
                }
            }
            AnyPacket::EquipEntity(request) => {
                if let Some((target, character)) = lookup.net_to_ecs(request.target_id)
                    .zip(lookup.net_to_ecs(request.character_id)) {
                    equip_events.send(EquipEvent {
                        client_entity,
                        target,
                        character,
                        slot: request.slot,
                    });
                }
            }
            AnyPacket::AsciiTextMessageRequest(request) => {
                chat_events.send(ChatRequestEvent {
                    client_entity,
                    request: UnicodeTextMessageRequest {
                        kind: request.kind,
                        hue: request.hue,
                        font: request.font,
                        text: request.text.clone(),
                        ..Default::default()
                    },
                });
            }
            AnyPacket::UnicodeTextMessageRequest(request) => {
                chat_events.send(ChatRequestEvent { client_entity, request: request.clone() });
            }
            AnyPacket::CharacterProfile(request) => {
                match request {
                    CharacterProfile::Request(request) => {
                        if let Some(target) = lookup.net_to_ecs(request.target_id) {
                            profile_events.send(ProfileEvent {
                                client_entity,
                                target,
                                new_profile: request.new_profile.clone(),
                            });
                        }
                    }
                    _ => unreachable!(),
                }
            }
            AnyPacket::EntityRequest(request) => {
                let target = match lookup.net_to_ecs(request.target) {
                    Some(x) => x,
                    _ => continue,
                };

                match request.kind {
                    EntityRequestKind::Skills => { skills_events.send(RequestSkillsEvent { client_entity, target }); }
                    kind => {
                        warn!("unhandled entity request: {kind:?}");
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn handle_tooltip_packets(
    lookup: Res<NetEntityLookup>,
    clients: Query<&NetClient>,
    mut tooltips: Query<&mut TooltipRequests>,
    mut events: EventReader<ReceivedPacketEvent>,
) {
    for ReceivedPacketEvent { client_entity: connection, packet } in events.read() {
        let connection = *connection;
        let AnyPacket::EntityTooltip(request) = packet else {
            continue;
        };

        let client = match clients.get(connection) {
            Ok(x) => x,
            _ => continue,
        };

        if let EntityTooltip::Request(ids) = request {
            for id in ids.iter().copied() {
                if let Some(entity) = lookup.net_to_ecs(id) {
                    if let Ok(mut tooltip) = tooltips.get_mut(entity) {
                        tooltip.requests.push(TooltipRequest {
                            client: connection,
                            entries: Vec::new(),
                        });
                    } else {
                        client.send_packet(EntityTooltip::Response {
                            id,
                            entries: Vec::new(),
                        });
                    }
                }
            }
        }
    }
}

pub fn send_tooltips(
    clients: Query<&NetClient>,
    mut tooltips: Query<(&NetId, &mut TooltipRequests), Changed<TooltipRequests>>,
) {
    for (net_id, mut tooltip) in &mut tooltips {
        if tooltip.requests.is_empty() {
            continue;
        }

        let id = net_id.id;
        for TooltipRequest { client, mut entries } in tooltip.requests.drain(..) {
            let client = match clients.get(client) {
                Ok(x) => x,
                _ => continue,
            };

            entries.sort();
            let entries = entries.into_iter()
                .map(|entry| {
                    EntityTooltipLine {
                        text_id: entry.text_id,
                        params: entry.arguments,
                    }
                })
                .collect();

            client.send_packet(EntityTooltip::Response { id, entries });
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<OwningClient>()
        .add_event::<ReceivedPacketEvent>()
        .add_systems(First, (
            (accept_new_clients, handle_new_packets)
                .chain()
                .in_set(ServerSet::Receive),
        ))
        .add_systems(First, (
            handle_login_packets,
            handle_input_packets,
            handle_tooltip_packets,
        ).in_set(ServerSet::HandlePackets))
        .add_systems(Last, (
            send_tooltips,
        ).in_set(ServerSet::Send));
}
