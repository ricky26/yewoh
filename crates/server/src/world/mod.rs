use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

use crate::world::events::{CharacterListEvent, ChatRequestEvent, ContextMenuEvent, CreateCharacterEvent, DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, NewPrimaryEntityEvent, PickUpEvent, ProfileEvent, ReceivedPacketEvent, RequestSkillsEvent, SelectCharacterEvent, SentPacketEvent, SingleClickEvent};
use crate::world::input::{handle_context_menu_packets, send_context_menu, update_targets};
use crate::world::net::{accept_new_clients, finish_synchronizing, update_view, handle_input_packets, handle_login_packets, handle_new_packets, MapInfos, NetEntityAllocator, NetEntityLookup, send_remove_entity, send_tooltips, send_updated_stats, start_synchronizing, sync_entities, update_characters, update_entity_lookup, update_equipped_items, update_items_in_containers, update_items_in_world, update_players, update_tooltips, send_hidden_entities};
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
            .add_event::<ContextMenuEvent>()
            .add_event::<ProfileEvent>()
            .add_event::<RequestSkillsEvent>()
            .add_event::<NewPrimaryEntityEvent>()
            .add_event::<ChatRequestEvent>()
            .add_systems((
                accept_new_clients,
                send_context_menu.before(handle_new_packets),
                send_tooltips.before(handle_new_packets),
                send_hidden_entities.before(handle_new_packets),
                update_players.before(handle_new_packets),
                send_updated_stats.before(handle_new_packets),
                update_items_in_world.before(handle_new_packets),
                update_items_in_containers.before(handle_new_packets),
                update_equipped_items.before(handle_new_packets),
                update_characters.before(handle_new_packets),
                handle_new_packets.after(accept_new_clients),
            ).in_base_set(CoreSet::First))
            .add_systems((
                start_synchronizing,
                update_view,
                handle_login_packets,
                handle_input_packets,
                handle_context_menu_packets,
            ).in_base_set(CoreSet::PreUpdate))
            .add_systems((
                sync_entities,
            ).in_base_set(CoreSet::Update))
            .add_systems((
                finish_synchronizing,
            ).in_base_set(CoreSet::PostUpdate))
            .add_systems((
                send_remove_entity.before(update_entity_lookup),
                update_targets,
                update_tooltips,
                update_entity_lookup,
                update_entity_surfaces,
                limit_tick_rate,
            ).in_base_set(CoreSet::Last));
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
