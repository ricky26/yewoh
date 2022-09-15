use bevy_ecs::prelude::*;

use yewoh::protocol::{MessageKind, Packet, UnicodeTextMessage};
use yewoh_server::world::entity::Stats;
use yewoh_server::world::events::ChatRequestEvent;
use yewoh_server::world::net::{broadcast, NetClient, NetEntity, NetOwned};

use crate::commands::TextCommandExecutor;

pub fn handle_incoming_chat(
    mut events: EventReader<ChatRequestEvent>,
    mut command_executor: TextCommandExecutor,
    clients: Query<(&NetClient, &NetOwned)>,
    character_query: Query<(&NetEntity, &Stats)>,
) {
    for ChatRequestEvent { client_entity: client, request } in events.iter() {
        if command_executor.try_split_exec(*client, &request.text) {
            continue;
        }

        let (_, owned) = match clients.get(*client) {
            Ok(x) => x,
            _ => continue,
        };

        let (net, stats) = match character_query.get(owned.primary_entity) {
            Ok(x) => x,
            _ => continue,
        };

        broadcast(clients.iter().map(|(c, _)| c), UnicodeTextMessage {
            entity_id: Some(net.id),
            kind: MessageKind::Regular,
            language: "ENG".to_string(),
            text: request.text.clone(),
            name: stats.name.to_string(),
            hue: 1234,
            font: 1,
            graphic_id: 0
        }.into_arc());
    }
}
