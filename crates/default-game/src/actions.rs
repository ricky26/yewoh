use bevy_ecs::prelude::*;
use yewoh::protocol::{MoveConfirm, OpenPaperDoll};
use yewoh_server::world::client::{NetClient, PlayerServer};
use yewoh_server::world::entity::{HasNotoriety, MapPosition, NetEntity};
use yewoh_server::world::events::{DoubleClickEvent, MoveEvent};

pub fn handle_move(
    mut server: ResMut<PlayerServer>,
    mut events: EventReader<MoveEvent>,
    connection_query: Query<&NetClient>,
    mut character_query: Query<(&mut MapPosition, &HasNotoriety)>,
) {
    for MoveEvent { connection, request } in events.iter() {
        let connection = *connection;
        let client_component = match connection_query.get(connection) {
            Ok(x) => x,
            _ => continue,
        };

        let client = match server.client_mut(connection) {
            Some(x) => x,
            None => continue,
        };

        let primary_entity = match client_component.primary_entity {
            Some(x) => x,
            None => continue,
        };

        let (mut map_position, notoriety) = match character_query.get_mut(primary_entity) {
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

pub fn handle_double_click(
    mut server: ResMut<PlayerServer>,
    mut events: EventReader<DoubleClickEvent>,
    target_query: Query<&NetEntity>,
) {
    for DoubleClickEvent { connection, target } in events.iter() {
        let connection = *connection;
        let target = match target {
            Some(x) => *x,
            None => continue,
        };

        let id = match target_query.get(target) {
            Ok(e) => e.id,
            _ => continue,
        };

        server.send_packet(connection, OpenPaperDoll {
            id,
            text: "Me, Myself and I".into(),
            flags: Default::default()
        }.into());
    }
}
