use bevy::prelude::*;
use smallvec::SmallVec;
use yewoh_server::world::gump::OnClientCloseGump;
use yewoh_server::world::ServerSet;

use crate::entity_events::{EntityEvent, EntityEventPlugin};

#[derive(Debug, Clone, Event)]
pub struct OnCloseGump {
    pub client_entity: Entity,
    pub gump: Entity,
    pub button_id: u32,
    pub on_switches: SmallVec<[u32; 16]>,
    pub text_fields: Vec<String>,
}

impl EntityEvent for OnCloseGump {
    fn target(&self) -> Entity {
        self.gump
    }
}

pub fn on_client_close_gump(
    mut events: EventReader<OnClientCloseGump>,
    mut out_events: EventWriter<OnCloseGump>,
) {
    for event in events.read() {
        out_events.send(OnCloseGump {
            client_entity: event.client_entity,
            gump: event.gump,
            button_id: event.button_id,
            on_switches: event.on_switches.clone(),
            text_fields: event.text_fields.clone(),
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventPlugin::<OnCloseGump>::default(),
        ))
        .add_systems(First, (
            on_client_close_gump.in_set(ServerSet::HandlePackets),
        ));
}
