use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use tokio::runtime::Handle;

use crate::world::client::{accept_new_clients, apply_new_primary_entities, handle_packets, MapInfos};
use crate::world::entity::NetEntityAllocator;
use crate::world::events::{CharacterListEvent, ChatRequestEvent, CreateCharacterEvent, MoveEvent, NewPrimaryEntityEvent};
use crate::world::time::{limit_tick_rate, TickRate};

pub mod time;

pub mod client;

pub mod entity;

pub mod events;

#[derive(Default)]
pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<TickRate>()
            .init_resource::<MapInfos>()
            .init_resource::<NetEntityAllocator>()
            .insert_resource(Handle::current())
            .add_event::<CharacterListEvent>()
            .add_event::<CreateCharacterEvent>()
            .add_event::<MoveEvent>()
            .add_event::<NewPrimaryEntityEvent>()
            .add_event::<ChatRequestEvent>()
            .add_system(accept_new_clients)
            .add_system(handle_packets.after(accept_new_clients))
            .add_system(apply_new_primary_entities)
            .add_system_to_stage(CoreStage::Last, limit_tick_rate);
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
