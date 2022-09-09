use bevy_ecs::prelude::*;
use yewoh::protocol::MoveConfirm;
use yewoh_server::world::client::PlayerServer;
use yewoh_server::world::entity::{HasNotoriety, MapPosition};
use yewoh_server::world::events::MoveEvent;

pub fn handle_move(
    mut server: ResMut<PlayerServer>,
    mut events: EventReader<MoveEvent>,
    mut query: Query<(&mut MapPosition, &HasNotoriety)>,
) {
    for MoveEvent { connection, primary_entity, request } in events.iter() {
        let client = match server.client_mut(*connection) {
            Some(x) => x,
            None => continue,
        };

        let primary_entity = match primary_entity {
            Some(x) => *x,
            None => continue,
        };

        let (mut map_position, notoriety) = match query.get_mut(primary_entity) {
            Ok(x) => x,
            _ => continue,
        };
        map_position.position += request.direction.as_vec2().extend(0);
        log::debug!("Move to {:?}", map_position);

        let notoriety = **notoriety;
        client.send_packet(MoveConfirm {
            sequence: request.sequence,
            notoriety,
        }.into());
    }
}