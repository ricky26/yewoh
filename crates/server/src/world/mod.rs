use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

use crate::world::events::{CharacterListEvent, ChatRequestEvent, ContextMenuEvent, CreateCharacterEvent, DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, PickUpEvent, ProfileEvent, ReceivedPacketEvent, RequestSkillsEvent, SelectCharacterEvent, SentPacketEvent, SingleClickEvent};
use crate::world::input::{handle_context_menu_packets, send_context_menu, update_targets};
use crate::world::net::{accept_new_clients, add_new_entities_to_lookup, finish_synchronizing, handle_input_packets, handle_login_packets, handle_new_packets, MapInfos, NetEntityAllocator, NetEntityLookup, remove_old_entities_from_lookup, send_change_map, send_ghost_updates, send_tooltips, start_synchronizing};
use crate::world::spatial::{EntityPositions, EntitySurfaces, update_entity_positions, update_entity_surfaces};

pub mod net;

pub mod entity;

pub mod events;

pub mod spatial;

pub mod map;

pub mod input;

#[derive(SystemSet, Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerSet {
    Receive,
    HandlePackets,
    UpdateVisibility,
    SendFirst,
    SendGhosts,
    Send,
    SendLast,
}

#[derive(Default)]
pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MapInfos>()
            .init_resource::<NetEntityAllocator>()
            .init_resource::<NetEntityLookup>()
            .init_resource::<EntitySurfaces>()
            .init_resource::<EntityPositions>()
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
            .add_event::<ChatRequestEvent>()
            .configure_sets((
                ServerSet::Receive.in_base_set(CoreSet::First),
                ServerSet::HandlePackets.in_base_set(CoreSet::First).after(ServerSet::Receive),
                ServerSet::UpdateVisibility
                    .in_base_set(CoreSet::PostUpdate)
                    .before(ServerSet::Send),
                ServerSet::SendFirst.in_base_set(CoreSet::Last),
                ServerSet::SendGhosts.in_base_set(CoreSet::Last).after(ServerSet::SendFirst),
                ServerSet::Send.in_base_set(CoreSet::Last).after(ServerSet::SendGhosts),
                ServerSet::SendLast.in_base_set(CoreSet::Last).after(ServerSet::Send),
            ))
            .add_systems(
                (accept_new_clients, handle_new_packets)
                    .chain()
                    .in_set(ServerSet::Receive))
            .add_systems((
                start_synchronizing,
                handle_login_packets,
                handle_input_packets,
                handle_context_menu_packets,
            ).in_set(ServerSet::HandlePackets))
            .add_systems((
                send_change_map,
                add_new_entities_to_lookup,
            ).in_set(ServerSet::SendFirst))
            /*.add_systems((
            ).in_set(ServerSet::SendGhosts))*/
            .add_systems((
                send_context_menu,
                send_tooltips,
                send_ghost_updates.before(finish_synchronizing),
                finish_synchronizing,
                update_targets,
            ).in_set(ServerSet::Send))
            .add_systems((
                remove_old_entities_from_lookup,
            ).in_set(ServerSet::SendLast))
            .add_systems((
                update_entity_surfaces,
                update_entity_positions,
            ).in_set(ServerSet::UpdateVisibility));
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
