use bevy_ecs::prelude::*;

use yewoh::protocol::{MoveConfirm, OpenContainer, OpenPaperDoll};
use yewoh_server::world::entity::{Character, Container, KnownPosition, MapPosition, Notorious};
use yewoh_server::world::events::{DoubleClickEvent, MoveEvent};
use yewoh_server::world::net::{NetClient, NetEntity, NetOwned};

pub fn handle_move(
    mut events: EventReader<MoveEvent>,
    connection_query: Query<(&NetClient, &NetOwned)>,
    mut character_query: Query<(&mut MapPosition, &mut KnownPosition, &Notorious)>,
) {
    for MoveEvent { client: connection, request } in events.iter() {
        let connection = *connection;
        let (client, owned) = match connection_query.get(connection) {
            Ok(x) => x,
            _ => continue,
        };

        let primary_entity = owned.primary_entity;
        let (mut map_position, mut known_position, notoriety) = match character_query.get_mut(primary_entity) {
            Ok(x) => x,
            _ => continue,
        };
        map_position.direction = request.direction;

        if request.sequence > 0 {
            map_position.position += request.direction.as_vec2().extend(0);
            log::debug!("Move to {:?}", map_position);
        }

        known_position.expected_position = Some(map_position.clone());

        let notoriety = **notoriety;
        client.send_packet(MoveConfirm {
            sequence: request.sequence,
            notoriety,
        }.into());
    }
}

pub fn handle_double_click(
    mut events: EventReader<DoubleClickEvent>,
    clients: Query<&NetClient>,
    target_query: Query<(&NetEntity, Option<&Character>, Option<&Container>)>,
) {
    for DoubleClickEvent { client, target } in events.iter() {
        let client = match clients.get(*client) {
            Ok(x) => x,
            _ => continue,
        };
        let target = match target {
            Some(x) => *x,
            None => continue,
        };

        let (net, character, container) = match target_query.get(target) {
            Ok(e) => e,
            _ => continue,
        };

        if character.is_some() {
            client.send_packet(OpenPaperDoll {
                id: net.id,
                text: "Me, Myself and I".into(),
                flags: Default::default()
            }.into());
        }

        if let Some(container) = container {
            client.send_packet(OpenContainer {
                id: net.id,
                gump_id: container.gump_id,
            }.into());
        }
    }
}
