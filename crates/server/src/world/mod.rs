use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use tokio::runtime::Handle;

use crate::world::net::{NetEntityAllocator, NetEntityLookup, MapInfos, accept_new_clients, start_synchronizing, handle_input_packets, handle_login_packets, handle_new_packets, send_remove_entity, update_containers, send_updated_stats, update_entity_lookup, update_items_in_containers, update_items_in_world, update_equipped_items, update_characters, update_players, finish_synchronizing, sync_entities};
use crate::world::events::{
    CharacterListEvent, ChatRequestEvent, CreateCharacterEvent, DoubleClickEvent, MoveEvent, NewPrimaryEntityEvent,
    ReceivedPacketEvent, SentPacketEvent, SingleClickEvent,
};
use crate::world::time::{limit_tick_rate, TickRate};

pub mod time;

pub mod net;

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
            .init_resource::<NetEntityLookup>()
            .insert_resource(Handle::current())
            .add_event::<ReceivedPacketEvent>()
            .add_event::<SentPacketEvent>()
            .add_event::<CharacterListEvent>()
            .add_event::<CreateCharacterEvent>()
            .add_event::<MoveEvent>()
            .add_event::<SingleClickEvent>()
            .add_event::<DoubleClickEvent>()
            .add_event::<NewPrimaryEntityEvent>()
            .add_event::<ChatRequestEvent>()
            .add_system_to_stage(CoreStage::First, accept_new_clients)
            .add_system_to_stage(CoreStage::First, update_players.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, send_updated_stats.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_items_in_world.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_items_in_containers.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_equipped_items.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_characters.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_containers.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, handle_new_packets.after(accept_new_clients))
            .add_system_to_stage(CoreStage::PreUpdate, start_synchronizing)
            .add_system_to_stage(CoreStage::PreUpdate, handle_login_packets)
            .add_system_to_stage(CoreStage::PreUpdate, handle_input_packets)
            .add_system_to_stage(CoreStage::Update, sync_entities)
            .add_system_to_stage(CoreStage::PostUpdate, finish_synchronizing)
            .add_system_to_stage(CoreStage::Last, send_remove_entity.before(update_entity_lookup))
            .add_system_to_stage(CoreStage::Last, update_entity_lookup)
            .add_system_to_stage(CoreStage::Last, limit_tick_rate);
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
