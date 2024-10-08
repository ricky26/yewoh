use crate::characters::Animation;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::{Event, EventReader};
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use yewoh::protocol::{CharacterAnimation, CharacterPredefinedAnimation};
use yewoh_server::world::entity::Location;
use yewoh_server::world::net::{NetClient, NetEntityLookup, Synchronized};
use yewoh_server::world::spatial::NetClientPositions;

#[derive(Debug, Clone, Event)]
pub struct AnimationStartedEvent {
    pub entity: Entity,
    pub location: Location,
    pub animation: Animation,
}

pub fn send_animations(
    entity_lookup: Res<NetEntityLookup>,
    client_positions: Res<NetClientPositions>,
    clients: Query<&NetClient, With<Synchronized>>,
    mut events: EventReader<AnimationStartedEvent>,
) {
    for event in events.read() {
        let target_id = match entity_lookup.ecs_to_net(event.entity) {
            Some(x) => x,
            None => continue,
        };

        for (client_entity, ..) in client_positions.tree.iter_at_point(event.location.map_id, event.location.position.truncate()) {
            let client = match clients.get(client_entity) {
                Ok(x) => x,
                _ => continue,
            };

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
