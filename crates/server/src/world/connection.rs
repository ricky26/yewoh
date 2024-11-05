use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use tokio::sync::mpsc;
use tracing::{info, trace, warn};
use yewoh::protocol::encryption::Encryption;
use yewoh::protocol::{AnyPacket, CharacterProfile, ClientVersion, ClientVersionRequest, EntityRequestKind, EntityTooltip, EntityTooltipLine, ExtendedCommand, FeatureFlags, GameServerLogin, IntoAnyPacket, SetAttackTarget, SupportedFeatures, UnicodeTextMessageRequest};

use crate::async_runtime::AsyncRuntime;
use crate::game_server::NewSessionAttempt;
use crate::lobby::{NewSessionRequest, SessionAllocator};
use crate::world::account::{CharacterListEvent, CreateCharacterEvent, DeleteCharacterEvent, SelectCharacterEvent, SentCharacterList, User};
use crate::world::characters::{ProfileEvent, RequestSkillsEvent, RequestStatusEvent};
use crate::world::chat::ChatRequestEvent;
use crate::world::combat::AttackRequestedEvent;
use crate::world::entity::{TooltipRequest, TooltipRequests};
use crate::world::input::{ContextMenuEvent, ContextMenuRequest, OnClientDoubleClick, DropEvent, EquipEvent, MoveEvent, PickUpEvent, OnClientSingleClick, Targeting, EntityTargetResponse, WorldTargetResponse};
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
    runtime: Res<AsyncRuntime>,
    mut server: ResMut<NetServer>,
    connections: Query<&NetClient>,
    mut commands: Commands,
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
                let result = match action {
                    WriterAction::Send(client_version, packet) => {
                        trace!("OUT ({address:?}): {packet:?}");
                        writer.send(client_version, &packet).await
                    }
                    WriterAction::SendArc(client_version, packet) => {
                        trace!("OUT ({address:?}): {packet:?}");
                        writer.send(client_version, &*packet).await
                    }
                };
                if let Err(err) = result {
                    if err.downcast_ref::<std::io::Error>()
                        .map_or(true, |e| e.kind() != ErrorKind::BrokenPipe) {
                        warn!("Error sending packet {err}");
                    }
                    break;
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

#[derive(SystemParam)]
pub struct NewPacketEvents<'w> {
    pub character_list_events: EventWriter<'w, CharacterListEvent>,
    pub character_creation_events: EventWriter<'w, CreateCharacterEvent>,
    pub select_character_events: EventWriter<'w, SelectCharacterEvent>,
    pub delete_character_events: EventWriter<'w, DeleteCharacterEvent>,
    pub move_events: EventWriter<'w, MoveEvent>,
    pub chat_events: EventWriter<'w, ChatRequestEvent>,
    pub pick_up_events: EventWriter<'w, PickUpEvent>,
    pub drop_events: EventWriter<'w, DropEvent>,
    pub equip_events: EventWriter<'w, EquipEvent>,
    pub profile_events: EventWriter<'w, ProfileEvent>,
    pub skills_events: EventWriter<'w, RequestSkillsEvent>,
    pub status_events: EventWriter<'w, RequestStatusEvent>,
    pub context_menu_events: EventWriter<'w, ContextMenuEvent>,
    pub attack_events: EventWriter<'w, AttackRequestedEvent>,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_new_packets(
    mut commands: Commands,
    mut server: ResMut<NetServer>,
    lookup: Res<NetEntityLookup>,
    mut clients: Query<(&NetClient, Option<&SentCharacterList>, Option<&mut Targeting>)>,
    mut events: NewPacketEvents,
    mut tooltips: Query<&mut TooltipRequests>,
) {
    while let Ok((client_entity, packet)) = server.received_packets_rx.try_recv() {
        let (client, sent_character_list, targeting) = match clients.get_mut(client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        match packet {
            // Login packets
            AnyPacket::ClientVersionRequest(_) => {
                if sent_character_list.is_none() {
                    commands.entity(client_entity).insert(SentCharacterList);

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
                    events.character_list_events.send(CharacterListEvent {
                        client_entity,
                    });
                }
            }
            AnyPacket::CreateCharacterClassic(create_character) => {
                events.character_creation_events.send(CreateCharacterEvent {
                    client_entity,
                    request: create_character.0,
                });
            }
            AnyPacket::CreateCharacterEnhanced(create_character) => {
                events.character_creation_events.send(CreateCharacterEvent {
                    client_entity,
                    request: create_character.0,
                });
            }
            AnyPacket::SelectCharacter(select_character) => {
                events.select_character_events.send(SelectCharacterEvent {
                    client_entity,
                    request: select_character,
                });
            }
            AnyPacket::DeleteCharacter(delete_character) => {
                events.delete_character_events.send(DeleteCharacterEvent {
                    client_entity,
                    request: delete_character,
                });
            }

            // Input packets
            AnyPacket::Move(request) => {
                events.move_events.send(MoveEvent { client_entity, request });
            }
            AnyPacket::SingleClick(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    commands.trigger_targets(OnClientSingleClick {
                        client_entity,
                        target,
                    }, client_entity);
                } else {
                    warn!("Single click for non-existent entity {:?}", request.target_id);
                }
            }
            AnyPacket::DoubleClick(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    commands.trigger_targets(OnClientDoubleClick {
                        client_entity,
                        target,
                    }, client_entity);
                } else {
                    warn!("Double click for non-existent entity {:?}", request.target_id);
                }
            }
            AnyPacket::PickUpEntity(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    events.pick_up_events.send(PickUpEvent {
                        client_entity,
                        target,
                    });
                }
            }
            AnyPacket::DropEntity(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    events.drop_events.send(DropEvent {
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
                    events.equip_events.send(EquipEvent {
                        client_entity,
                        target,
                        character,
                        slot: request.slot,
                    });
                }
            }
            AnyPacket::CharacterProfile(request) => {
                match request {
                    CharacterProfile::Request(request) => {
                        if let Some(target) = lookup.net_to_ecs(request.target_id) {
                            events.profile_events.send(ProfileEvent {
                                client_entity,
                                target,
                                new_profile: request.new_profile,
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
                    EntityRequestKind::Status => { events.status_events.send(RequestStatusEvent { client_entity, target }); }
                    EntityRequestKind::Skills => { events.skills_events.send(RequestSkillsEvent { client_entity, target }); }
                }
            }

            // Chat packets
            AnyPacket::AsciiTextMessageRequest(request) => {
                events.chat_events.send(ChatRequestEvent {
                    client_entity,
                    request: UnicodeTextMessageRequest {
                        kind: request.kind,
                        hue: request.hue,
                        font: request.font,
                        text: request.text,
                        ..Default::default()
                    },
                });
            }
            AnyPacket::UnicodeTextMessageRequest(request) => {
                events.chat_events.send(ChatRequestEvent { client_entity, request });
            }

            AnyPacket::EntityTooltip(request) => {
                if let EntityTooltip::Request(ids) = request {
                    for id in ids.iter().copied() {
                        if let Some(entity) = lookup.net_to_ecs(id) {
                            if let Ok(mut tooltip) = tooltips.get_mut(entity) {
                                tooltip.requests.push(TooltipRequest {
                                    client: client_entity,
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

            AnyPacket::ExtendedCommand(packet) => {
                match packet {
                    ExtendedCommand::ContextMenuRequest(target_id) => {
                        let target = match lookup.net_to_ecs(target_id) {
                            Some(x) => x,
                            _ => continue,
                        };

                        commands.spawn(ContextMenuRequest {
                            client_entity,
                            target,
                            entries: vec![],
                        });
                    }
                    ExtendedCommand::ContextMenuResponse(response) => {
                        let target = match lookup.net_to_ecs(response.target_id) {
                            Some(x) => x,
                            _ => continue,
                        };

                        events.context_menu_events.send(ContextMenuEvent {
                            client_entity,
                            target,
                            option: response.id,
                        });
                    }
                    p => {
                        debug!("unhandled extended packet {:?}", p);
                    }
                }
            }

            AnyPacket::PickTarget(request) => {
                let Some(mut targeting) = targeting else {
                    return;
                };

                if let Some(pending) = targeting.pending.front() {
                    let mut entity = commands.entity(pending.request_entity);

                    if request.target_ground {
                        entity.insert(WorldTargetResponse {
                            position: Some(request.position),
                        });
                    } else {
                        entity.insert(EntityTargetResponse {
                            target: request.target_id.and_then(|id| lookup.net_to_ecs(id)),
                        });
                    }

                    targeting.pending.pop_front();
                    if let Some(next) = targeting.pending.front() {
                        client.send_packet(next.packet.clone());
                    }
                }
            }

            AnyPacket::AttackRequest(packet) => {
                let target = match lookup.net_to_ecs(packet.target_id) {
                    Some(x) => x,
                    None => {
                        client.send_packet(SetAttackTarget {
                            target_id: None,
                        });
                        continue;
                    }
                };

                events.attack_events.send(AttackRequestedEvent {
                    client_entity,
                    target,
                });
            }

            _ => {}
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
        .add_systems(First, (
            (accept_new_clients, handle_new_packets)
                .chain()
                .in_set(ServerSet::Receive),
        ))
        .add_systems(Last, (
            send_tooltips,
        ).in_set(ServerSet::Send));
}
