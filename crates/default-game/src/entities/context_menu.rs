use bevy::prelude::*;
use yewoh::protocol;
use yewoh::protocol::{ContextMenu, ContextMenuFlags, ExtendedCommand};
use yewoh_server::world::connection::NetClient;
use yewoh_server::world::input::{OnClientContextMenuAction, OnClientContextMenuRequest};
use yewoh_server::world::net_id::NetId;
use yewoh_server::world::ServerSet;

use crate::entity_events::{EntityEvent, EntityEventPlugin, EntityEventReader, EntityEventRoutePlugin};
use crate::DefaultGameSet;
use crate::entities::interactions::OnEntitySingleClick;

#[derive(Debug, Clone, Default, Reflect)]
pub struct ContextMenuEntry {
    pub id: u16,
    pub text_id: u32,
    pub hue: Option<u16>,
    pub disabled: bool,
    pub highlighted: bool,
    pub arrow: bool,
    pub priority: u32,
}

impl ContextMenuEntry {
    pub fn flags(&self) -> ContextMenuFlags {
        let mut flags = ContextMenuFlags::empty();

        if self.disabled {
            flags |= ContextMenuFlags::DISABLED;
        }

        if self.highlighted {
            flags |= ContextMenuFlags::HIGHLIGHTED;
        }

        if self.arrow {
            flags |= ContextMenuFlags::ARROW;
        }

        if self.hue.is_some() {
            flags |= ContextMenuFlags::HUE;
        }

        flags
    }
}

#[derive(Clone, Debug, Event)]
pub struct OnEntityContextMenuRequest {
    pub client_entity: Entity,
    pub target: Entity,
    pub entries: Vec<ContextMenuEntry>,
}

impl EntityEvent for OnEntityContextMenuRequest {
    fn target(&self) -> Entity {
        self.target
    }
}

#[derive(Clone, Debug, Event)]
pub struct OnEntityContextMenuAction {
    pub client_entity: Entity,
    pub target: Entity,
    pub id: u16,
}

impl EntityEvent for OnEntityContextMenuAction {
    fn target(&self) -> Entity {
        self.target
    }
}

#[derive(Clone, Debug, Default, Component, Reflect)]
#[reflect(Component)]
pub struct SingleClickContextMenu;

pub fn on_client_context_menu_request(
    mut events: EventReader<OnClientContextMenuRequest>,
    mut out_events: EventWriter<OnEntityContextMenuRequest>,
) {
    for request in events.read() {
        out_events.send(OnEntityContextMenuRequest {
            client_entity: request.client_entity,
            target: request.target,
            entries: Vec::new(),
        });
    }
}

pub fn finish_context_menu(
    clients: Query<&NetClient>,
    net_objects: Query<&NetId>,
    mut events: EntityEventReader<OnEntityContextMenuRequest, ()>,
) {
    for event in events.read() {
        let Ok(client) = clients.get(event.client_entity) else {
            continue;
        };

        let Ok(net_id) = net_objects.get(event.target) else {
            continue;
        };

        event.entries.sort_by_key(|l| (l.priority, l.id, l.text_id));
        let entries = event.entries.drain(..)
            .map(|l| protocol::ContextMenuEntry {
                id: l.id,
                text_id: l.text_id,
                hue: l.hue,
                flags: l.flags(),
            })
            .collect();

        client.send_packet(ExtendedCommand::ContextMenu(ContextMenu {
            target_id: net_id.id,
            entries,
        }));
    }
}

pub fn on_client_context_menu_action(
    mut events: EventReader<OnClientContextMenuAction>,
    mut out_events: EventWriter<OnEntityContextMenuAction>,
) {
    for request in events.read() {
        out_events.send(OnEntityContextMenuAction {
            client_entity: request.client_entity,
            target: request.target,
            id: request.action_id,
        });
    }
}

pub fn show_context_menu_on_single_click(
    mut events: EntityEventReader<OnEntitySingleClick, SingleClickContextMenu>,
    mut events_out: EventWriter<OnClientContextMenuRequest>,
) {
    for event in events.read() {
        events_out.send(OnClientContextMenuRequest {
            client_entity: event.client_entity,
            target: event.target,
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<SingleClickContextMenu>()
        .add_plugins((
            EntityEventPlugin::<OnEntityContextMenuRequest>::default(),
            EntityEventPlugin::<OnEntityContextMenuAction>::default(),
            EntityEventRoutePlugin::<OnEntityContextMenuRequest, ()>::default(),
            EntityEventRoutePlugin::<OnEntitySingleClick, SingleClickContextMenu>::default(),
        ))
        .add_systems(First, (
            (
                on_client_context_menu_request,
                on_client_context_menu_action,
                show_context_menu_on_single_click,
            ).in_set(ServerSet::HandlePackets),
            finish_context_menu.in_set(DefaultGameSet::FinishEvents),
        ));
}
