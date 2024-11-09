use bevy::app::App;
use bevy::prelude::*;
use yewoh::protocol::OpenPaperDoll;
use yewoh::types::FixedString;
use yewoh_server::world::connection::NetClient;
use yewoh_server::world::net_id::NetId;

use crate::DefaultGameSet;
use crate::entities::context_menu::{ContextMenuEntry, OnEntityContextMenuAction, OnEntityContextMenuRequest};
use crate::entities::interactions::OnEntityDoubleClick;
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};

const PAPERDOLL_ID: u16 = 1;

#[derive(Clone, Debug, Default, Component, Reflect)]
#[reflect(Component)]
pub struct Paperdoll;

#[derive(Clone, Debug, Default, Component, Reflect)]
#[reflect(Component)]
pub struct DoubleClickPaperdoll;

pub fn paperdoll_context_menu(
    mut events: EntityEventReader<OnEntityContextMenuRequest, Paperdoll>,
) {
    for event in events.read() {
        event.entries.push(ContextMenuEntry {
            id: PAPERDOLL_ID,
            text_id: 3006123,
            ..default()
        });
    }
}

pub fn paperdoll_context_menu_action(
    clients: Query<&NetClient>,
    net_objects: Query<&NetId>,
    mut events: EntityEventReader<OnEntityContextMenuAction, Paperdoll>,
) {
    for event in events.read() {
        if event.id != PAPERDOLL_ID {
            continue;
        }

        let Ok(client) = clients.get(event.client_entity) else {
            continue;
        };

        let Ok(net_id) = net_objects.get(event.target) else {
            continue;
        };

        client.send_packet(OpenPaperDoll {
            id: net_id.id,
            text: FixedString::from_str("Me, Myself and I"),
            flags: Default::default(),
        });
    }
}

pub fn paperdoll_double_click(
    clients: Query<&NetClient>,
    net_objects: Query<&NetId>,
    mut events: EntityEventReader<OnEntityDoubleClick, DoubleClickPaperdoll>,
) {
    for event in events.read() {
        let Ok(client) = clients.get(event.client_entity) else {
            continue;
        };

        let Ok(net_id) = net_objects.get(event.target) else {
            continue;
        };

        client.send_packet(OpenPaperDoll {
            id: net_id.id,
            text: FixedString::from_str("Me, Myself and I"),
            flags: Default::default(),
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<Paperdoll>()
        .register_type::<DoubleClickPaperdoll>()
        .add_plugins((
            EntityEventRoutePlugin::<OnEntityDoubleClick, DoubleClickPaperdoll>::default(),
            EntityEventRoutePlugin::<OnEntityContextMenuRequest, Paperdoll>::default(),
            EntityEventRoutePlugin::<OnEntityContextMenuAction, Paperdoll>::default(),
        ))
        .add_systems(First, (
            (
                paperdoll_context_menu,
                paperdoll_context_menu_action,
                paperdoll_double_click,
            ).in_set(DefaultGameSet::HandleEvents),
        ));
}
