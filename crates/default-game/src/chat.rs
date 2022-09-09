use bevy_ecs::prelude::*;
use glam::IVec3;
use yewoh::Direction;
use yewoh::protocol::AsciiTextMessage;
use yewoh_server::world::client::{NetClient, NetClients};
use yewoh_server::world::entity::{Graphic, MapPosition, NetEntity, NetEntityAllocator};
use yewoh_server::world::events::ChatRequestEvent;

pub fn handle_incoming_chat(
    mut events: EventReader<ChatRequestEvent>,
    clients: Res<NetClients>,
    allocator: Res<NetEntityAllocator>,
    connection_query: Query<&NetClient>,
    character_query: Query<&MapPosition>,
    mut commands: Commands,
) {
    for ChatRequestEvent { connection, request } in events.iter() {
        log::info!("{:?}: {}", connection, &request.text);
        clients.send_packet(*connection, AsciiTextMessage {
            text: "hello".to_string(),
            name: "hello2".to_string(),
            hue: 120,
            font: 3,
            ..Default::default()
        }.into());

        let connection_component = match connection_query.get(*connection) {
            Ok(x) => x,
            _ => continue,
        };

        let character_entity = match connection_component.primary_entity {
            Some(x) => x,
            None => continue,
        };

        let position = match character_query.get(character_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let id = allocator.allocate_item();
        commands.spawn()
            .insert(NetEntity { id })
            .insert(MapPosition {
                map_id: 1,
                position: position.position + IVec3::new(5, -4, 0),
                direction: Direction::North,
            })
            .insert(Graphic {
                id: 0x97f,
                hue: 0x7d0,
            });
    }
}
