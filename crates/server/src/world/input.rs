use std::collections::VecDeque;

use bevy::prelude::*;
use yewoh::protocol::{PickTarget, TargetType};

use crate::world::connection::NetClient;
use crate::world::entity::{Direction, EquipmentSlot};
use crate::world::ServerSet;

#[derive(Debug, Clone, Event)]
pub struct OnClientMove {
    pub client_entity: Entity,
    pub direction: Direction,
    pub run: bool,
    pub sequence: u8,
    pub fast_walk: u32,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientSingleClick {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientDoubleClick {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientPickUp {
    pub client_entity: Entity,
    pub target: Entity,
    pub quantity: u16,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientDrop {
    pub client_entity: Entity,
    pub target: Entity,
    pub position: IVec3,
    pub grid_index: u8,
    pub dropped_on: Option<Entity>,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientEquip {
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
pub struct InFlightTargetRequest {
    pub request_entity: Entity,
    pub packet: PickTarget,
}

#[derive(Debug, Clone, Default, Component)]
pub struct Targeting {
    pub pending: VecDeque<InFlightTargetRequest>,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientContextMenuRequest {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientContextMenuAction {
    pub client_entity: Entity,
    pub target: Entity,
    pub action_id: u16,
}

#[allow(clippy::too_many_arguments)]
pub fn update_targets(
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
    mut commands: Commands,
) {
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
            client.send_packet(packet.clone());
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
                client.send_packet(next.packet.clone());
            }
        } else {
            targeting.pending.remove(position);
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_event::<OnClientMove>()
        .add_event::<OnClientSingleClick>()
        .add_event::<OnClientDoubleClick>()
        .add_event::<OnClientPickUp>()
        .add_event::<OnClientDrop>()
        .add_event::<OnClientEquip>()
        .add_event::<OnClientContextMenuRequest>()
        .add_event::<OnClientContextMenuAction>()
        .add_systems(Last, (
            update_targets,
        ).in_set(ServerSet::Send));
}
