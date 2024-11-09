use bevy::prelude::*;
use yewoh_server::world::items::OnContainerOpen;

use crate::DefaultGameSet;
use crate::entities::interactions::OnEntityDoubleClick;
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};

#[derive(Clone, Debug, Default, Component, Reflect)]
#[reflect(Component)]
pub struct DoubleClickOpenContainer;

pub fn open_containers(
    mut events: EntityEventReader<OnEntityDoubleClick, DoubleClickOpenContainer>,
    mut out_events: EventWriter<OnContainerOpen>,
) {
    for event in events.read() {
        out_events.send(OnContainerOpen {
            client_entity: event.client_entity,
            container: event.target,
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<DoubleClickOpenContainer>()
        .add_plugins((
            EntityEventRoutePlugin::<OnEntityDoubleClick, DoubleClickOpenContainer>::default(),
        ))
        .add_systems(First, (
            open_containers.in_set(DefaultGameSet::HandleEvents),
        ));
}
