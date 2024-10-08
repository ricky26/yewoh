use bevy_ecs::prelude::RemovedComponents;
use bevy_ecs::query::Changed;
use bevy_ecs::system::{Query, Res};
use yewoh::protocol::SetAttackTarget;

use crate::world::entity::AttackTarget;
use crate::world::net::{NetClient, NetEntityLookup, NetOwner};

pub fn send_updated_attack_target(
    entity_lookup: Res<NetEntityLookup>,
    clients: Query<&NetClient>,
    owners: Query<&NetOwner>,
    modified_targets: Query<(&NetOwner, &AttackTarget), Changed<AttackTarget>>,
    mut removed_targets: RemovedComponents<AttackTarget>,
) {
    for (owner, attack_target) in &modified_targets {
        let client = match clients.get(owner.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let target_id = entity_lookup.ecs_to_net(attack_target.target);
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
