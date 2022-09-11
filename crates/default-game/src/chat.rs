use bevy_ecs::prelude::*;
use glam::IVec3;

use yewoh::Direction;
use yewoh::protocol::AsciiTextMessage;
use yewoh_server::world::entity::{Graphic, MapPosition};
use yewoh_server::world::events::ChatRequestEvent;
use yewoh_server::world::net::{NetClient, NetEntity, NetEntityAllocator, NetOwned};

pub fn handle_incoming_chat(
    mut events: EventReader<ChatRequestEvent>,
    allocator: Res<NetEntityAllocator>,
    clients: Query<(&NetClient, &NetOwned)>,
    character_query: Query<&MapPosition>,
    mut commands: Commands,
) {
    for ChatRequestEvent { client, .. } in events.iter() {
        let (client, owned) = match clients.get(*client) {
            Ok(x) => x,
            _ => continue,
        };

        client.send_packet(AsciiTextMessage {
            text: "hello".to_string(),
            name: "hello2".to_string(),
            hue: 120,
            font: 3,
            ..Default::default()
        }.into());

        let position = match character_query.get(owned.primary_entity) {
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
