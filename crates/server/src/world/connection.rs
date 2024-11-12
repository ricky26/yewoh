use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::fmt::Debug;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, trace, warn};
use yewoh::protocol::encryption::Encryption;
use yewoh::protocol::{AnyPacket, ClientVersion, ClientVersionRequest, EntityRequestKind, ExtendedCommand, FeatureFlags, GameServerLogin, IntoAnyPacket, SetAttackTarget, SupportedFeatures, UnicodeTextMessageRequest, ViewRange};

use crate::async_runtime::AsyncRuntime;
use crate::game_server::NewSessionAttempt;
use crate::lobby::{NewSessionRequest, SessionAllocator};
use crate::world::account::{OnClientCharacterListRequest, OnClientCreateCharacter, OnClientDeleteCharacter, OnClientSelectCharacter, SentCharacterList, User};
use crate::world::characters::{OnClientProfileRequest, OnClientProfileUpdateRequest, OnClientSkillsRequest, OnClientStatusRequest};
use crate::world::chat::OnClientChatMessage;
use crate::world::combat::{OnClientAttackRequest, OnClientWarModeChanged};
use crate::world::entity::{EquipmentSlot, OnClientTooltipRequest};
use crate::world::gump::{GumpIdAllocator, GumpLookup, GumpSent, OnClientCloseGump};
use crate::world::input::{EntityTargetResponse, OnClientContextMenuAction, OnClientContextMenuRequest, OnClientDoubleClick, OnClientDrop, OnClientEquip, OnClientMove, OnClientPickUp, OnClientSingleClick, Targeting, WorldTargetResponse};
use crate::world::net_id::NetEntityLookup;
use crate::world::view::{View, MAX_VIEW_RANGE, MIN_VIEW_RANGE};
use crate::world::ServerSet;

pub enum WriterAction {
    Send(ClientVersion, AnyPacket),
    SendArc(ClientVersion, Arc<AnyPacket>),
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Possessing {
    pub entity: Entity,
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct OwningClient {
    pub client_entity: Entity,
}

#[derive(Debug, Clone, Component)]
#[require(Targeting, View, GumpIdAllocator)]
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
    pub character_list_request: EventWriter<'w, OnClientCharacterListRequest>,
    pub create_character: EventWriter<'w, OnClientCreateCharacter>,
    pub select_character: EventWriter<'w, OnClientSelectCharacter>,
    pub delete_character: EventWriter<'w, OnClientDeleteCharacter>,
    pub move_request: EventWriter<'w, OnClientMove>,
    pub single_click: EventWriter<'w, OnClientSingleClick>,
    pub double_click: EventWriter<'w, OnClientDoubleClick>,
    pub pick_up: EventWriter<'w, OnClientPickUp>,
    pub drop: EventWriter<'w, OnClientDrop>,
    pub equip: EventWriter<'w, OnClientEquip>,
    pub profile_update: EventWriter<'w, OnClientProfileUpdateRequest>,
    pub profile_request: EventWriter<'w, OnClientProfileRequest>,
    pub status_request: EventWriter<'w, OnClientStatusRequest>,
    pub skills_request: EventWriter<'w, OnClientSkillsRequest>,
    pub chat_message: EventWriter<'w, OnClientChatMessage>,
    pub tooltip_request: EventWriter<'w, OnClientTooltipRequest>,
    pub context_menu_request: EventWriter<'w, OnClientContextMenuRequest>,
    pub context_menu_action: EventWriter<'w, OnClientContextMenuAction>,
    pub war_mode: EventWriter<'w, OnClientWarModeChanged>,
    pub attack: EventWriter<'w, OnClientAttackRequest>,
    pub close_gump: EventWriter<'w, OnClientCloseGump>,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_new_packets(
    mut commands: Commands,
    mut server: ResMut<NetServer>,
    lookup: Res<NetEntityLookup>,
    gumps: ResMut<GumpLookup>,
    mut clients: Query<
        (&NetClient, &mut View, Option<&SentCharacterList>, &mut Targeting),
    >,
    mut events: NewPacketEvents,
) {
    while let Ok((client_entity, packet)) = server.received_packets_rx.try_recv() {
        let Ok((client, mut view, sent_character_list, mut targeting)) = clients.get_mut(client_entity) else {
            continue;
        };

        match packet {
            // Login packets
            AnyPacket::ClientVersionRequest(_) => {
                if sent_character_list.is_some() {
                    continue;
                }

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

                events.character_list_request.send(OnClientCharacterListRequest {
                    client_entity,
                });
            }
            AnyPacket::CreateCharacterClassic(request) => {
                events.create_character.send(OnClientCreateCharacter {
                    client_entity,
                    request: request.0,
                });
            }
            AnyPacket::CreateCharacterEnhanced(request) => {
                events.create_character.send(OnClientCreateCharacter {
                    client_entity,
                    request: request.0,
                });
            }
            AnyPacket::SelectCharacter(request) => {
                events.select_character.send(OnClientSelectCharacter {
                    client_entity,
                    request,
                });
            }
            AnyPacket::DeleteCharacter(request) => {
                events.delete_character.send(OnClientDeleteCharacter {
                    client_entity,
                    request,
                });
            }

            // Input packets
            AnyPacket::Move(request) => {
                events.move_request.send(OnClientMove {
                    client_entity,
                    direction: request.direction.into(),
                    run: request.run,
                    sequence: request.sequence,
                    fast_walk: request.fast_walk,
                });
            }
            AnyPacket::SingleClick(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    events.single_click.send(OnClientSingleClick {
                        client_entity,
                        target,
                    });
                } else {
                    warn!("Single click for non-existent entity {:?}", request.target_id);
                }
            }
            AnyPacket::DoubleClick(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    events.double_click.send(OnClientDoubleClick {
                        client_entity,
                        target,
                    });
                } else {
                    warn!("Double click for non-existent entity {:?}", request.target_id);
                }
            }
            AnyPacket::PickUpEntity(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    events.pick_up.send(OnClientPickUp {
                        client_entity,
                        target,
                        quantity: request.quantity,
                    });
                }
            }
            AnyPacket::DropEntity(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    events.drop.send(OnClientDrop {
                        client_entity,
                        target,
                        position: request.position,
                        grid_index: request.grid_index,
                        dropped_on: request.dropped_on_entity_id.and_then(|id| lookup.net_to_ecs(id)),
                    });
                }
            }
            AnyPacket::EquipEntity(request) => {
                if let Some((target, character)) = lookup.net_to_ecs(request.target_id)
                    .zip(lookup.net_to_ecs(request.character_id)) {
                    if let Some(slot) = EquipmentSlot::from_protocol(request.slot) {
                        events.equip.send(OnClientEquip {
                            client_entity,
                            target,
                            character,
                            slot,
                        });
                    } else {
                        warn!("invalid equipment slot on equip packet");
                    }
                }
            }
            AnyPacket::ProfileRequest(request) => {
                if let Some(target) = lookup.net_to_ecs(request.target_id) {
                    if let Some(new_profile) = request.new_profile {
                        events.profile_update.send(OnClientProfileUpdateRequest {
                            client_entity,
                            target,
                            new_profile,
                        });
                    } else {
                        events.profile_request.send(OnClientProfileRequest {
                            client_entity,
                            target,
                        });
                    }
                }
            }
            AnyPacket::EntityRequest(request) => {
                let target = match lookup.net_to_ecs(request.target) {
                    Some(x) => x,
                    _ => continue,
                };

                match request.kind {
                    EntityRequestKind::Status => {
                        events.status_request.send(OnClientStatusRequest {
                            client_entity,
                            target,
                        });
                    }
                    EntityRequestKind::Skills => {
                        events.skills_request.send(OnClientSkillsRequest {
                            client_entity,
                            target,
                        });
                    }
                }
            }

            // Chat packets
            AnyPacket::AsciiTextMessageRequest(request) => {
                events.chat_message.send(OnClientChatMessage {
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
                events.chat_message.send(OnClientChatMessage {
                    client_entity,
                    request,
                });
            }

            AnyPacket::EntityTooltipRequest(request) => {
                let targets = request.entity_ids.into_iter()
                    .filter_map(|id| lookup.net_to_ecs(id))
                    .collect();
                events.tooltip_request.send(OnClientTooltipRequest {
                    client_entity,
                    targets,
                });
            }

            AnyPacket::ExtendedCommand(packet) => {
                match packet {
                    ExtendedCommand::ContextMenuRequest(target_id) => {
                        let target = match lookup.net_to_ecs(target_id) {
                            Some(x) => x,
                            _ => continue,
                        };

                        events.context_menu_request.send(OnClientContextMenuRequest {
                            client_entity,
                            target,
                        });
                    }
                    ExtendedCommand::ContextMenuResponse(response) => {
                        let target = match lookup.net_to_ecs(response.target_id) {
                            Some(x) => x,
                            _ => continue,
                        };

                        events.context_menu_action.send(OnClientContextMenuAction {
                            client_entity,
                            target,
                            action_id: response.id,
                        });
                    }
                    p => {
                        debug!("unhandled extended packet {:?}", p);
                    }
                }
            }

            AnyPacket::PickTarget(request) => {
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

            AnyPacket::WarMode(war_mode) => {
                events.war_mode.send(OnClientWarModeChanged {
                    client_entity,
                    war_mode: war_mode.war,
                });
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

                events.attack.send(OnClientAttackRequest {
                    client_entity,
                    target,
                });
            }

            // UI
            AnyPacket::GumpResult(result) => {
                if let Some(gump) = gumps.get_gump(client_entity, result.gump_id, result.type_id) {
                    commands.entity(gump).remove::<GumpSent>();
                    events.close_gump.send(OnClientCloseGump {
                        client_entity,
                        gump,
                        button_id: result.button_id,
                        on_switches: result.on_switches,
                        text_fields: result.text_fields,
                    });
                } else {
                    warn!("result for unknown gump {} {} for client {client_entity}",
                        result.gump_id, result.type_id);
                }
            }

            AnyPacket::ViewRange(packet) => {
                let new_view_range = packet.0
                    .min(MAX_VIEW_RANGE as u8)
                    .max(MIN_VIEW_RANGE as u8);
                if new_view_range != packet.0 {
                    client.send_packet(ViewRange(new_view_range));
                }
                view.range = new_view_range as i32;
            }

            _ => {}
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<OwningClient>()
        .register_type::<Possessing>()
        .add_systems(First, (
            (accept_new_clients, handle_new_packets)
                .chain()
                .in_set(ServerSet::Receive),
        ));
}
