use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use crate::world::entity::{AttackTarget, Character, Container, EquippedBy, Flags, Graphic, Location, Multi, Notorious, ParentContainer, Quantity, Stats, Tooltip};

use crate::world::events::{AttackRequestedEvent, CharacterListEvent, ChatRequestEvent, ContextMenuEvent, CreateCharacterEvent, DeleteCharacterEvent, DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, PickUpEvent, ProfileEvent, ReceivedPacketEvent, RequestSkillsEvent, SelectCharacterEvent, SentPacketEvent, SingleClickEvent};
use crate::world::input::{handle_attack_packets, handle_context_menu_packets, send_context_menu, update_targets};
use crate::world::net::{accept_new_clients, add_new_entities_to_lookup, ContainerOpenedEvent, finish_synchronizing, handle_input_packets, handle_login_packets, handle_new_packets, MapInfos, NetEntityAllocator, NetEntityLookup, observe_ghosts, remove_old_entities_from_lookup, send_change_map, send_ghost_updates, send_opened_containers, send_tooltips, send_updated_attack_target, start_synchronizing};
use crate::world::spatial::{EntityPositions, EntitySurfaces, NetClientPositions, update_client_positions, update_entity_positions, update_entity_surfaces};

pub mod net;

pub mod entity;

pub mod events;

pub mod spatial;

pub mod navigation;

pub mod map;

pub mod input;

pub mod hierarchy;

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
            .init_resource::<NetClientPositions>()
            .register_type::<Flags>()
            .register_type::<Notorious>()
            .register_type::<Character>()
            .register_type::<Quantity>()
            .register_type::<Graphic>()
            .register_type::<Multi>()
            .register_type::<Location>()
            .register_type::<Container>()
            .register_type::<ParentContainer>()
            .register_type::<EquippedBy>()
            .register_type::<Stats>()
            .register_type::<Tooltip>()
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
            ).in_set(ServerSet::HandlePackets))
            .add_systems(Last, (
                send_change_map,
                add_new_entities_to_lookup,
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
            .add_systems(Last, (
                remove_old_entities_from_lookup,
            ).in_set(ServerSet::SendLast))
            .add_systems(PostUpdate, (
                update_entity_surfaces,
                update_entity_positions,
                update_client_positions,
            ).in_set(ServerSet::UpdateVisibility));
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
