use bevy_ecs::prelude::*;
use glam::IVec2;

use yewoh::Direction;
use yewoh::protocol::{EntityFlags, GumpLayout, MessageKind, OpenGump, Packet, UnicodeTextMessage};
use yewoh_server::world::entity::{Flags, Graphic, MapPosition};
use yewoh_server::world::events::ChatRequestEvent;
use yewoh_server::world::net::{broadcast, NetClient, NetEntity, NetEntityAllocator, NetOwned};

use crate::commands::TextCommandExecutor;

pub fn handle_incoming_chat(
    mut events: EventReader<ChatRequestEvent>,
    allocator: Res<NetEntityAllocator>,
    mut command_executor: TextCommandExecutor,
    clients: Query<(&NetClient, &NetOwned)>,
    character_query: Query<(&NetEntity, &MapPosition)>,
    mut commands: Commands,
) {
    for ChatRequestEvent { client, request } in events.iter() {
        if command_executor.try_split_exec(*client, &request.text) {
            continue;
        }

        let (client, owned) = match clients.get(*client) {
            Ok(x) => x,
            _ => continue,
        };

        let (net, position) = match character_query.get(owned.primary_entity) {
            Ok(x) => x,
            _ => continue,
        };

        broadcast(clients.iter().map(|(c, _)| c), UnicodeTextMessage {
            entity_id: Some(net.id),
            kind: MessageKind::Regular,
            language: "ENG".to_string(),
            text: request.text.clone(),
            name: "User".to_string(),
            hue: 1234,
            font: 1,
            graphic_id: 0
        }.into_arc());

        let id = allocator.allocate_item();
        commands.spawn()
            .insert(NetEntity { id })
            .insert(Flags { flags: EntityFlags::default() })
            .insert(MapPosition {
                map_id: 1,
                position: position.position,
                direction: Direction::North,
            })
            .insert(Graphic {
                id: 0x97f,
                hue: 0x7d0,
            });

        client.send_packet(OpenGump {
            id: 1,
            type_id: 2,
            position: IVec2::new(50, 50),
            layout: GumpLayout {
                layout: "{ page 0 }{ resizepic 0 0 5054 420 440 }{ text 0 0 120 0 }".to_string(),
                text: vec!["Hello, world!".into()],
            },
        }.into());
    }
}
