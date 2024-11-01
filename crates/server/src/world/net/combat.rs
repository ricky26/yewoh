use bevy::ecs::query::Changed;
use bevy::ecs::removal_detection::RemovedComponents;
use bevy::ecs::system::Query;
use yewoh::protocol::SetAttackTarget;

use crate::world::entity::AttackTarget;
use crate::world::net::{NetClient, NetId, OwningClient};

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
        }.into());
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
        }.into());
    }
}
