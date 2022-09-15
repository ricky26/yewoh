use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use tokio::runtime::Handle;

use crate::world::events::{CharacterListEvent, ChatRequestEvent, CreateCharacterEvent, DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, NewPrimaryEntityEvent, PickUpEvent, ReceivedPacketEvent, SelectCharacterEvent, SentPacketEvent, SingleClickEvent};
use crate::world::input::update_targets;
use crate::world::net::{accept_new_clients, finish_synchronizing, handle_input_packets, handle_login_packets, handle_new_packets, MapInfos, NetEntityAllocator, NetEntityLookup, send_remove_entity, send_tooltips, send_updated_stats, start_synchronizing, sync_entities, update_characters, update_entity_lookup, update_equipped_items, update_items_in_containers, update_items_in_world, update_players, update_tooltips};
use crate::world::spatial::{EntitySurfaces, update_entity_surfaces};
use crate::world::time::{limit_tick_rate, Tick, TickRate};

pub mod time;

pub mod net;

pub mod entity;

pub mod events;

pub mod spatial;

pub mod map;

pub mod input;

#[derive(Default)]
pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<Tick>()
            .init_resource::<TickRate>()
            .init_resource::<MapInfos>()
            .init_resource::<NetEntityAllocator>()
            .init_resource::<NetEntityLookup>()
            .init_resource::<EntitySurfaces>()
            .insert_resource(Handle::current())
            .add_event::<ReceivedPacketEvent>()
            .add_event::<SentPacketEvent>()
            .add_event::<CharacterListEvent>()
            .add_event::<CreateCharacterEvent>()
            .add_event::<SelectCharacterEvent>()
            .add_event::<MoveEvent>()
            .add_event::<SingleClickEvent>()
            .add_event::<DoubleClickEvent>()
            .add_event::<PickUpEvent>()
            .add_event::<DropEvent>()
            .add_event::<EquipEvent>()
            .add_event::<NewPrimaryEntityEvent>()
            .add_event::<ChatRequestEvent>()
            .add_system_to_stage(CoreStage::First, accept_new_clients)
            .add_system_to_stage(CoreStage::First, send_tooltips.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_players.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, send_updated_stats.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_items_in_world.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_items_in_containers.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_equipped_items.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, update_characters.before(handle_new_packets))
            .add_system_to_stage(CoreStage::First, handle_new_packets.after(accept_new_clients))
            .add_system_to_stage(CoreStage::PreUpdate, start_synchronizing)
            .add_system_to_stage(CoreStage::PreUpdate, handle_login_packets)
            .add_system_to_stage(CoreStage::PreUpdate, handle_input_packets)
            .add_system_to_stage(CoreStage::Update, sync_entities)
            .add_system_to_stage(CoreStage::PostUpdate, finish_synchronizing)
            .add_system_to_stage(CoreStage::Last, send_remove_entity.before(update_entity_lookup))
            .add_system_to_stage(CoreStage::Last, update_targets)
            .add_system_to_stage(CoreStage::Last, update_tooltips)
            .add_system_to_stage(CoreStage::Last, update_entity_lookup)
            .add_system_to_stage(CoreStage::Last, update_entity_surfaces)
            .add_system_to_stage(CoreStage::Last, limit_tick_rate);
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
