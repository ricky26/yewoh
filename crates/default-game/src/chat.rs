use bevy::prelude::*;

use yewoh::protocol::{MessageKind, UnicodeTextMessage};
use yewoh::types::FixedString;
use yewoh_server::world::characters::CharacterName;
use yewoh_server::world::chat::OnClientChatMessage;
use yewoh_server::world::connection::{broadcast, NetClient, Possessing};
use yewoh_server::world::net_id::{NetId};

use crate::commands::TextCommandExecutor;

pub fn on_client_chat_message(
    mut command_executor: TextCommandExecutor,
    clients: Query<(&NetClient, &Possessing)>,
    character_query: Query<(&NetId, &CharacterName)>,
    mut events: EventReader<OnClientChatMessage>,
) {
    for request in events.read() {
        if command_executor.try_split_exec(request.client_entity, &request.request.text) {
            continue;
        }

        let Ok((_, owned)) = clients.get(request.client_entity) else {
            continue;
        };

        let Ok((net, name)) = character_query.get(owned.entity) else {
            continue;
        };

        broadcast(clients.iter().map(|(c, _)| c), UnicodeTextMessage {
            entity_id: Some(net.id),
            kind: MessageKind::Regular,
            language: FixedString::from_str("ENG"),
            text: request.request.text.clone(),
            name: FixedString::from_str(name.as_str()),
            hue: 1234,
            font: 1,
            graphic_id: 0,
        });
    }
}
