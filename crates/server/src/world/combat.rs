use bevy::ecs::entity::{VisitEntities, VisitEntitiesMut};
use bevy::ecs::reflect::{ReflectMapEntities, ReflectVisitEntities, ReflectVisitEntitiesMut};
use bevy::prelude::*;
use yewoh::protocol::{DamageDealt, IntoAnyPacket, SetAttackTarget, Swing};

use crate::world::connection::{NetClient, OwningClient};
use crate::world::delta_grid::{delta_grid_cell, DeltaEntry, DeltaGrid, DeltaVersion};
use crate::world::entity::RootPosition;
use crate::world::net_id::NetId;
use crate::world::ServerSet;

#[derive(Debug, Clone, Eq, PartialEq, Reflect, Component, VisitEntities, VisitEntitiesMut)]
#[reflect(Component, VisitEntities, VisitEntitiesMut, MapEntities)]
pub struct AttackTarget {
    pub target: Entity,
}

impl FromWorld for AttackTarget {
    fn from_world(_world: &mut World) -> Self {
        AttackTarget {
            target: Entity::PLACEHOLDER,
        }
    }
}

#[derive(Debug, Clone, Event)]
pub struct AttackRequestedEvent {
    pub client_entity: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Reflect, Event)]
pub struct DamagedEvent {
    pub target: Entity,
    pub damage: u16,
}

#[derive(Debug, Clone, Reflect, Event)]
pub struct SwingEvent {
    pub attacker: Entity,
    pub target: Entity,
}

pub fn send_updated_attack_target(
    net_ids: Query<&NetId>,
    clients: Query<&NetClient>,
    owners: Query<&OwningClient>,
    modified_targets: Query<(&OwningClient, &AttackTarget), Changed<AttackTarget>>,
    mut removed_targets: RemovedComponents<AttackTarget>,
) {
    for (owner, attack_target) in &modified_targets {
        let client = match clients.get(owner.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let target_id = net_ids.get(attack_target.target).ok().map(|id| id.id);
        client.send_packet(SetAttackTarget {
            target_id,
        });
    }

    for entity in removed_targets.read() {
        let owner = match owners.get(entity) {
            Ok(x) => x,
            _ => continue,
        };

        let client = match clients.get(owner.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        client.send_packet(SetAttackTarget {
            target_id: None,
        });
    }
}

pub fn detect_damage_notices(
    delta_version: Res<DeltaVersion>,
    mut delta_grid: ResMut<DeltaGrid>,
    damage_targets: Query<(&NetId, &RootPosition)>,
    mut damage_events: EventReader<DamagedEvent>,
) {
    for event in damage_events.read() {
        let Ok((target_id, position)) = damage_targets.get(event.target) else {
            warn!("Damage event on non-net entity {}", event.target);
            continue;
        };

        let grid_cell = delta_grid_cell(position.position.truncate());
        let packet = DamageDealt {
            target_id: target_id.id,
            damage: event.damage,
        }.into_any_arc();

        if let Some(cell) = delta_grid.cell_at_mut(position.map_id, grid_cell) {
            cell.deltas.push(delta_version.new_delta(DeltaEntry::CharacterDamaged {
                entity: event.target,
                packet,
            }));
        }
    }
}

pub fn detect_swings(
    delta_version: Res<DeltaVersion>,
    mut delta_grid: ResMut<DeltaGrid>,
    characters: Query<(&NetId, &RootPosition)>,
    mut swing_events: EventReader<SwingEvent>,
) {
    for event in swing_events.read() {
        let Ok((attacker_id, position)) = characters.get(event.attacker) else {
            warn!("Swing attacker is non-net entity {}", event.target);
            continue;
        };

        let Ok((target_id, _)) = characters.get(event.target) else {
            warn!("Swing target is non-net entity {}", event.target);
            continue;
        };

        let grid_cell = delta_grid_cell(position.position.truncate());
        let packet = Swing {
            target_id: target_id.id,
            attacker_id: attacker_id.id,
        }.into_any_arc();

        if let Some(cell) = delta_grid.cell_at_mut(position.map_id, grid_cell) {
            cell.deltas.push(delta_version.new_delta(DeltaEntry::CharacterSwing {
                entity: event.attacker,
                target: event.target,
                packet,
            }));
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<AttackTarget>()
        .add_event::<AttackRequestedEvent>()
        .add_systems(Last, (
            send_updated_attack_target
                .in_set(ServerSet::SendLast),
            (
                detect_damage_notices,
                detect_swings,
            ).in_set(ServerSet::DetectChanges)
        ));
}
