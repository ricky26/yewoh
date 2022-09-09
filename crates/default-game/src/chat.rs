use bevy_ecs::prelude::*;
use yewoh::protocol::AsciiTextMessage;
use yewoh_server::world::client::NetClients;
use yewoh_server::world::events::ChatRequestEvent;

pub fn handle_incoming_chat(
    mut events: EventReader<ChatRequestEvent>,
    server: Res<NetClients>,
) {
    for ChatRequestEvent { connection, request } in events.iter() {
        log::info!("{:?}: {}", connection, &request.text);
        server.send_packet(*connection, AsciiTextMessage {
            text: "hello".to_string(),
            name: "hello2".to_string(),
            hue: 120,
            font: 3,
            ..Default::default()
        }.into());
    }
}
