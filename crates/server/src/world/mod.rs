use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use tokio::runtime::Handle;

use crate::world::client::{accept_new_clients, handle_packets};
use crate::world::time::{limit_tick_rate, TickRate};

pub mod time;

pub mod client;

pub mod entity;

#[derive(Default)]
pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<TickRate>()
            .insert_resource(Handle::current())
            .add_system(accept_new_clients)
            .add_system(handle_packets.after(accept_new_clients))
            .add_system_to_stage(CoreStage::Last, limit_tick_rate);
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
