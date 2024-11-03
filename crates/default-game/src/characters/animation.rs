use bevy::prelude::*;
use yewoh::protocol::{CharacterAnimation, CharacterPredefinedAnimation};
use yewoh_server::world::connection::NetClient;
use yewoh_server::world::entity::MapPosition;
use yewoh_server::world::net_id::NetId;
use yewoh_server::world::view::Synchronized;

use crate::characters::Animation;

#[derive(Debug, Clone, Event)]
pub struct AnimationStartedEvent {
    pub entity: Entity,
    pub location: MapPosition,
    pub animation: Animation,
}

pub fn send_animations(
    net_ids: Query<&NetId>,
    clients: Query<&NetClient, With<Synchronized>>,
    mut events: EventReader<AnimationStartedEvent>,
) {
    for event in events.read() {
        let target_id = match net_ids.get(event.entity) {
            Ok(x) => x.id,
            _ => continue,
        };

        // TODO: filter these
        for client in &clients {
            let packet = match &event.animation {
                Animation::Inline(animation) => CharacterAnimation {
                    target_id,
                    animation_id: animation.animation_id,
                    frame_count: animation.frame_count,
                    repeat_count: animation.repeat_count,
                    reverse: animation.reverse,
                    speed: animation.speed,
                }.into(),
                Animation::Predefined(animation) => CharacterPredefinedAnimation {
                    target_id,
                    kind: animation.kind,
                    action: animation.action,
                    variant: animation.variant,
                }.into(),
            };
            client.send_packet(packet);
        }
    }
}
