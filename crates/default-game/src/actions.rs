use bevy_ecs::prelude::*;
use yewoh::protocol::{MoveConfirm, OpenContainer, OpenPaperDoll};
use yewoh_server::world::client::{NetClient, NetClients};
use yewoh_server::world::entity::{Character, Container, HasNotoriety, MapPosition, NetEntity};
use yewoh_server::world::events::{DoubleClickEvent, MoveEvent};

pub fn handle_move(
    server: Res<NetClients>,
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

        let client = match server.client(connection) {
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
        map_position.direction = request.direction;
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
    server: Res<NetClients>,
    mut events: EventReader<DoubleClickEvent>,
    target_query: Query<(&NetEntity, Option<&Character>, Option<&Container>)>,
) {
    for DoubleClickEvent { connection, target } in events.iter() {
        let connection = *connection;
        let target = match target {
            Some(x) => *x,
            None => continue,
        };

        let (net, character, container) = match target_query.get(target) {
            Ok(e) => e,
            _ => continue,
        };

        if character.is_some() {
            server.send_packet(connection, OpenPaperDoll {
                id: net.id,
                text: "Me, Myself and I".into(),
                flags: Default::default()
            }.into());
        }

        if let Some(container) = container {
            server.send_packet(connection, OpenContainer {
                id: net.id,
                gump_id: container.gump_id,
            }.into());
        }
    }
}
