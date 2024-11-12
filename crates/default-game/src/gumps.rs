use bevy::prelude::*;
use smallvec::SmallVec;
use yewoh_server::world::gump::OnClientCloseGump;
use yewoh_server::world::ServerSet;

use crate::entity_events::{EntityEvent, EntityEventPlugin};

pub const RESIZABLE_PAPER_1: u16 = 0x62b;
pub const RESIZABLE_PAPER_2: u16 = 0xbb8;
pub const RESIZABLE_PAPER_3: u16 = 0xdac;
pub const RESIZABLE_PAPER_4: u16 = 0x2454;
pub const RESIZABLE_PAPER_5: u16 = 0x2486;
pub const RESIZABLE_PAPER_6: u16 = 0x2ef8;
pub const RESIZABLE_DARK: u16 = 0xe10;
pub const RESIZABLE_TOOLTIP: u16 = 0xa3c;
pub const RESIZABLE_SCROLL_1: u16 = 0x9d8;
pub const RESIZABLE_SCROLL_2: u16 = 0x141e;
pub const RESIZABLE_SCROLL_3: u16 = 0x1432;
pub const RESIZABLE_SCROLL_4: u16 = 0x24a4;
pub const RESIZABLE_SCROLL_5: u16 = 0x24ae;
pub const RESIZABLE_STONE_1: u16 = 0xa28;
pub const RESIZABLE_STONE_2: u16 = 0x13be;
pub const RESIZABLE_STONE_3: u16 = 0x1400;
pub const RESIZABLE_STONE_4: u16 = 0x23f0;
pub const RESIZABLE_STONE_5: u16 = 0x2436;
pub const RESIZABLE_METAL_1: u16 = 0x53;
pub const RESIZABLE_METAL_2: u16 = 0x12e;
pub const RESIZABLE_METAL_3: u16 = 0x7748;
pub const RESIZABLE_METAL_4: u16 = 0x9bf5;
pub const RESIZABLE_METAL_5: u16 = 0x9c40;
pub const RESIZABLE_METAL_6: u16 = 0x9d60;
pub const RESIZABLE_WOOD_1: u16 = 0x6db;

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
