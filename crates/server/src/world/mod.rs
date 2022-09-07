use std::time::Duration;

use bevy_app::prelude::*;
use crate::world::client::{accept_new_clients, PlayerServer};

use crate::world::time::{limit_tick_rate, TickRate};

pub mod time;

pub mod client;

#[derive(Default)]
pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<TickRate>()
            .init_resource::<PlayerServer>()
            .add_system_to_stage(CoreStage::Last, limit_tick_rate)
            .add_system(accept_new_clients);
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
