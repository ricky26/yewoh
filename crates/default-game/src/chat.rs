use bevy::prelude::*;

use yewoh::protocol::{MessageKind, Packet, UnicodeTextMessage};
use yewoh::types::FixedString;
use yewoh_server::world::characters::CharacterName;
use yewoh_server::world::chat::ChatRequestEvent;
use yewoh_server::world::connection::{broadcast, NetClient, Possessing};
use yewoh_server::world::net_id::{NetId};

use crate::commands::TextCommandExecutor;

pub fn handle_incoming_chat(
    mut events: EventReader<ChatRequestEvent>,
    mut command_executor: TextCommandExecutor,
    clients: Query<(&NetClient, &Possessing)>,
    character_query: Query<(&NetId, &CharacterName)>,
) {
    for ChatRequestEvent { client_entity: client, request } in events.read() {
        if command_executor.try_split_exec(*client, &request.text) {
            continue;
        }

        let (_, owned) = match clients.get(*client) {
            Ok(x) => x,
            _ => continue,
        };

        let (net, name) = match character_query.get(owned.entity) {
            Ok(x) => x,
            _ => continue,
        };

        broadcast(clients.iter().map(|(c, _)| c), UnicodeTextMessage {
            entity_id: Some(net.id),
            kind: MessageKind::Regular,
            language: FixedString::from_str("ENG"),
            text: request.text.clone(),
            name: FixedString::from_str(name.as_str()),
            hue: 1234,
            font: 1,
            graphic_id: 0
        }.into_arc());
    }
}
