use std::collections::VecDeque;
use bevy_ecs::prelude::*;
use glam::IVec3;
use yewoh::protocol::{PickTarget, TargetType};
use crate::world::events::ReceivedPacketEvent;

use crate::world::net::{NetClient, NetEntityLookup};

#[derive(Debug, Clone, Component)]
pub struct WorldTargetRequest {
    pub connection: Entity,
    pub target_type: TargetType,
}

#[derive(Debug, Clone, Component)]
pub struct WorldTargetResponse {
    pub position: Option<IVec3>,
}

#[derive(Debug, Clone, Component)]
pub struct EntityTargetRequest {
    pub connection: Entity,
    pub target_type: TargetType,
}

#[derive(Debug, Clone, Component)]
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
    removed_world_targets: RemovedComponents<WorldTargetRequest>,
    removed_entity_targets: RemovedComponents<EntityTargetRequest>,
    mut events: EventReader<ReceivedPacketEvent>,
    mut commands: Commands,
) {
    for ReceivedPacketEvent { client: connection, packet } in events.iter() {
        let connection = *connection;
        let request = match packet.downcast::<PickTarget>() {
            Some(x) => x,
            None => continue,
        };

        let (client, mut targeting) = match clients.get_mut(connection) {
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
        .map(|(entity, request)| (entity, request.connection, false, request.target_type))
        .chain(
            new_world_targets.iter()
                .map(|(entity, request)| (entity, request.connection, true, request.target_type)));
    for (entity, connection, target_ground, target_type) in new_targets {
        let (client, mut targeting) = match clients.get_mut(connection) {
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
            id: entity.id(),
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

    for entity in removed_world_targets.iter().chain(removed_entity_targets.iter()) {
        let connection = match all_targets.get(entity) {
            Ok((_, Some(r), _)) => r.connection,
            Ok((_, _, Some(r))) => r.connection,
            _ => continue,
        };

        let (client, mut targeting) = match clients.get_mut(connection) {
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
