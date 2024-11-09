use bevy::prelude::*;

use yewoh_server::world::input::{OnClientDoubleClick, OnClientSingleClick};
use yewoh_server::world::ServerSet;

use crate::entity_events::{EntityEvent, EntityEventPlugin};

#[derive(Clone, Debug, Event)]
pub struct OnEntitySingleClick {
    pub client_entity: Entity,
    pub target: Entity,
}

impl EntityEvent for OnEntitySingleClick {
    fn target(&self) -> Entity {
        self.target
    }
}

#[derive(Clone, Debug, Event)]
pub struct OnEntityDoubleClick {
    pub client_entity: Entity,
    pub target: Entity,
}

impl EntityEvent for OnEntityDoubleClick {
    fn target(&self) -> Entity {
        self.target
    }
}

pub fn on_client_single_click(
    mut events: EventReader<OnClientSingleClick>,
    mut out_events: EventWriter<OnEntitySingleClick>,
) {
    for request in events.read() {
        out_events.send(OnEntitySingleClick {
            client_entity: request.client_entity,
            target: request.target,
        });
    }
}

pub fn on_client_double_click(
    mut events: EventReader<OnClientDoubleClick>,
    mut out_events: EventWriter<OnEntityDoubleClick>,
) {
    for request in events.read() {
        out_events.send(OnEntityDoubleClick {
            client_entity: request.client_entity,
            target: request.target,
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventPlugin::<OnEntitySingleClick>::default(),
            EntityEventPlugin::<OnEntityDoubleClick>::default(),
        ))
        .add_systems(First, (
            (
                on_client_single_click,
                on_client_double_click,
            ).in_set(ServerSet::HandlePackets),
        ));
}
