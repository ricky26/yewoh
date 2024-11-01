use bevy::prelude::*;

use crate::world::entity::{AttackTarget, BodyType, Container, ContainerPosition, EquippedPosition, Flags, Graphic, Hue, MapPosition, Multi, Notorious, Quantity, Stats, Tooltip, TooltipLine, TooltipRequests};
use crate::world::events::{AttackRequestedEvent, CharacterListEvent, ChatRequestEvent, ContextMenuEvent, CreateCharacterEvent, DeleteCharacterEvent, DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, PickUpEvent, ProfileEvent, ReceivedPacketEvent, RequestSkillsEvent, SelectCharacterEvent, SentPacketEvent, SingleClickEvent};
use crate::world::input::{handle_attack_packets, handle_context_menu_packets, send_context_menu, update_targets};
use crate::world::net::{accept_new_clients, assign_net_ids, finish_synchronizing, handle_input_packets, handle_login_packets, handle_new_packets, handle_tooltip_packets, observe_ghosts, send_change_map, send_ghost_updates, send_opened_containers, send_tooltips, send_updated_attack_target, start_synchronizing, ContainerOpenedEvent, MapInfos, NetEntityLookup, NetIdAllocator};
use crate::world::spatial::{update_client_positions, update_entity_positions, update_entity_surfaces, EntityPositions, EntitySurfaces, NetClientPositions};

pub mod net;

pub mod entity;

pub mod events;

pub mod spatial;

pub mod navigation;

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
            .add_plugins((
                net::plugin,
            ))
            .init_resource::<MapInfos>()
            .init_resource::<NetIdAllocator>()
            .init_resource::<NetEntityLookup>()
            .init_resource::<EntitySurfaces>()
            .init_resource::<EntityPositions>()
            .init_resource::<NetClientPositions>()
            .register_type::<Flags>()
            .register_type::<Notorious>()
            .register_type::<BodyType>()
            .register_type::<Quantity>()
            .register_type::<Graphic>()
            .register_type::<Hue>()
            .register_type::<Multi>()
            .register_type::<MapPosition>()
            .register_type::<Container>()
            .register_type::<ContainerPosition>()
            .register_type::<EquippedPosition>()
            .register_type::<Stats>()
            .register_type::<Tooltip>()
            .register_type::<TooltipRequests>()
            .register_type::<TooltipLine>()
            .register_type_data::<Vec<TooltipLine>, ReflectFromReflect>()
            .register_type::<AttackTarget>()
            .add_event::<ReceivedPacketEvent>()
            .add_event::<SentPacketEvent>()
            .add_event::<CharacterListEvent>()
            .add_event::<CreateCharacterEvent>()
            .add_event::<SelectCharacterEvent>()
            .add_event::<DeleteCharacterEvent>()
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
            .add_event::<AttackRequestedEvent>()
            .add_event::<ContainerOpenedEvent>()
            .configure_sets(First, (
                ServerSet::Receive,
                ServerSet::HandlePackets.after(ServerSet::Receive),
            ))
            .configure_sets(PostUpdate, (
                ServerSet::UpdateVisibility,
            ))
            .configure_sets(Last, (
                ServerSet::SendFirst,
                ServerSet::SendGhosts.after(ServerSet::SendFirst),
                ServerSet::Send.after(ServerSet::SendGhosts),
                ServerSet::SendLast.after(ServerSet::Send),
            ))
            .add_systems(First, (
                (accept_new_clients, handle_new_packets)
                    .chain()
                    .in_set(ServerSet::Receive),
            ))
            .add_systems(First, (
                start_synchronizing,
                handle_login_packets,
                handle_input_packets,
                handle_context_menu_packets,
                handle_attack_packets,
                handle_tooltip_packets,
            ).in_set(ServerSet::HandlePackets))
            .add_systems(Last, (
                send_change_map,
                assign_net_ids,
            ).in_set(ServerSet::SendFirst))
            .add_systems(Last, (
                observe_ghosts,
            ).in_set(ServerSet::SendGhosts))
            .add_systems(Last, (
                send_context_menu,
                send_tooltips,
                send_ghost_updates.before(finish_synchronizing),
                send_updated_attack_target.after(send_ghost_updates),
                send_opened_containers.after(send_ghost_updates),
                finish_synchronizing,
                update_targets,
            ).in_set(ServerSet::Send))
            .add_systems(PostUpdate, (
                update_entity_surfaces,
                update_entity_positions,
                update_client_positions,
            ).in_set(ServerSet::UpdateVisibility));
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
