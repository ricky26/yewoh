use std::collections::VecDeque;

use bevy::prelude::*;
use tracing::debug;
use yewoh::protocol::{AttackRequest, ContextMenu, ContextMenuEntry, EquipmentSlot, ExtendedCommand, Move, PickTarget, SetAttackTarget, TargetType};

use crate::world::combat::AttackRequestedEvent;
use crate::world::connection::{NetClient, ReceivedPacketEvent};
use crate::world::net_id::{NetEntityLookup, NetId};
use crate::world::ServerSet;

#[derive(Debug, Clone, Event)]
pub struct MoveEvent {
    pub client_entity: Entity,
    pub request: Move,
}

#[derive(Debug, Clone, Event)]
pub struct SingleClickEvent {
    pub client_entity: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct DoubleClickEvent {
    pub client_entity: Entity,
    pub target: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct PickUpEvent {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct DropEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub position: IVec3,
    pub grid_index: u8,
    pub dropped_on: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct EquipEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub character: Entity,
    pub slot: EquipmentSlot,
}

#[derive(Debug, Clone, Component)]
pub struct WorldTargetRequest {
    pub client_entity: Entity,
    pub target_type: TargetType,
}

#[derive(Debug, Clone, Component, Reflect)]
pub struct WorldTargetResponse {
    pub position: Option<IVec3>,
}

#[derive(Debug, Clone, Component)]
pub struct EntityTargetRequest {
    pub client_entity: Entity,
    pub target_type: TargetType,
}

#[derive(Debug, Clone, Component, Reflect)]
pub struct EntityTargetResponse {
    pub target: Option<Entity>,
}

#[derive(Debug, Clone)]
struct InFlightTargetRequest {
    request_entity: Entity,
    packet: PickTarget,
}

#[derive(Debug, Clone, Default, Component)]
pub struct Targeting {
    pending: VecDeque<InFlightTargetRequest>,
}

#[derive(Debug, Clone, Component)]
pub struct ContextMenuRequest {
    pub client_entity: Entity,
    pub target: Entity,
    pub entries: Vec<ContextMenuEntry>,
}

impl FromWorld for ContextMenuRequest {
    fn from_world(_world: &mut World) -> Self {
        ContextMenuRequest {
            client_entity: Entity::PLACEHOLDER,
            target: Entity::PLACEHOLDER,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Event)]
pub struct ContextMenuEvent {
    pub client_entity: Entity,
    pub target: Entity,
    pub option: u16,
}

pub fn update_targets(
    lookup: Res<NetEntityLookup>,
    mut clients: Query<(&NetClient, &mut Targeting)>,
    new_world_targets: Query<
        (Entity, &WorldTargetRequest),
        (Changed<WorldTargetRequest>, Without<WorldTargetResponse>),
    >,
    new_entity_targets: Query<
        (Entity, &EntityTargetRequest),
        (Changed<EntityTargetRequest>, Without<EntityTargetResponse>),
    >,
    all_targets: Query<
        (Entity, Option<&WorldTargetRequest>, Option<&EntityTargetRequest>),
        (Without<WorldTargetResponse>, Without<EntityTargetResponse>),
    >,
    mut removed_world_targets: RemovedComponents<WorldTargetRequest>,
    mut removed_entity_targets: RemovedComponents<EntityTargetRequest>,
    mut events: EventReader<ReceivedPacketEvent>,
    mut commands: Commands,
) {
    for ReceivedPacketEvent { client_entity, packet } in events.read() {
        let client_entity = *client_entity;
        let request = match packet.downcast::<PickTarget>() {
            Some(x) => x,
            None => continue,
        };

        let (client, mut targeting) = match clients.get_mut(client_entity) {
            Ok(x) => x,
            _ => continue,
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
                client.send_packet(next.packet.clone().into());
            }
        }
    }

    let new_targets = new_entity_targets.iter()
        .map(|(entity, request)| (entity, request.client_entity, false, request.target_type))
        .chain(
            new_world_targets.iter()
                .map(|(entity, request)| (entity, request.client_entity, true, request.target_type)));
    for (entity, client_entity, target_ground, target_type) in new_targets {
        let (client, mut targeting) = match clients.get_mut(client_entity) {
            Ok(x) => x,
            _ => {
                commands.entity(entity)
                    .insert(WorldTargetResponse {
                        position: None,
                    });
                continue;
            }
        };

        if targeting.pending.iter().any(|x| x.request_entity == entity) {
            continue;
        }

        let packet = PickTarget {
            target_ground,
            target_type,
            id: entity.index(),
            ..Default::default()
        };

        if targeting.pending.is_empty() {
            client.send_packet(packet.clone().into());
        }

        targeting.pending.push_back(InFlightTargetRequest {
            request_entity: entity,
            packet,
        });
    }

    for entity in removed_world_targets.read().chain(removed_entity_targets.read()) {
        let client_entity = match all_targets.get(entity) {
            Ok((_, Some(r), _)) => r.client_entity,
            Ok((_, _, Some(r))) => r.client_entity,
            _ => continue,
        };

        let (client, mut targeting) = match clients.get_mut(client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let position = match targeting.pending.iter().position(|i| i.request_entity == entity) {
            Some(x) => x,
            _ => continue,
        };

        if position == 0 {
            targeting.pending.pop_front();
            if let Some(next) = targeting.pending.front() {
                client.send_packet(next.packet.clone().into());
            }
        } else {
            targeting.pending.remove(position);
        }
    }
}

pub fn handle_context_menu_packets(
    lookup: Res<NetEntityLookup>,
    mut events: EventReader<ReceivedPacketEvent>,
    mut invoked_events: EventWriter<ContextMenuEvent>,
    mut commands: Commands,
) {
    for ReceivedPacketEvent { client_entity: client, packet } in events.read() {
        let client_entity = *client;
        let packet = match packet.downcast::<ExtendedCommand>() {
            Some(x) => x,
            _ => continue,
        };

        match packet {
            ExtendedCommand::ContextMenuRequest(target_id) => {
                let target = match lookup.net_to_ecs(*target_id) {
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

                invoked_events.send(ContextMenuEvent {
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
}

pub fn send_context_menu(
    clients: Query<&NetClient>,
    requests: Query<(Entity, &ContextMenuRequest)>,
    net_ids: Query<&NetId>,
    mut commands: Commands,
) {
    for (entity, request) in requests.iter() {
        commands.entity(entity).despawn();

        let client = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let target_id = match net_ids.get(request.target) {
            Ok(x) => x.id,
            _ => continue,
        };

        client.send_packet(ExtendedCommand::ContextMenuEnhanced(ContextMenu {
            target_id,
            entries: request.entries.clone(),
        }).into());
    }
}

pub fn handle_attack_packets(
    lookup: Res<NetEntityLookup>,
    clients: Query<&NetClient>,
    mut events: EventReader<ReceivedPacketEvent>,
    mut invoked_events: EventWriter<AttackRequestedEvent>,
) {
    for ReceivedPacketEvent { client_entity: client, packet } in events.read() {
        let client_entity = *client;
        let packet = match packet.downcast::<AttackRequest>() {
            Some(x) => x,
            _ => continue,
        };

        let target = match lookup.net_to_ecs(packet.target_id) {
            Some(x) => x,
            None => {
                if let Ok(client) = clients.get(client_entity) {
                    client.send_packet(SetAttackTarget {
                        target_id: None,
                    }.into());
                }

                continue;
            }
        };

        invoked_events.send(AttackRequestedEvent {
            client_entity,
            target,
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_event::<MoveEvent>()
        .add_event::<SingleClickEvent>()
        .add_event::<DoubleClickEvent>()
        .add_event::<PickUpEvent>()
        .add_event::<DropEvent>()
        .add_event::<EquipEvent>()
        .add_event::<ContextMenuEvent>()
        .add_systems(First, (
            handle_context_menu_packets,
            handle_attack_packets,
        ).in_set(ServerSet::HandlePackets))
        .add_systems(Last, (
            send_context_menu,
            update_targets,
        ).in_set(ServerSet::Send));
}
