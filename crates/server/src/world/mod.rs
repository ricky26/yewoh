use bevy::prelude::*;

pub mod entity;

pub mod position;

pub mod character;

pub mod item;

pub mod events;

pub mod spatial;

pub mod delta_grid;

pub mod navigation;

pub mod map;

pub mod input;

pub mod combat;

pub mod net_id;

pub mod connection;

pub mod view;

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
                net_id::plugin,
                connection::plugin,
                map::plugin,
                spatial::plugin,
                entity::plugin,
                events::plugin,
                view::plugin,
                input::plugin,
                combat::plugin,
                delta_grid::plugin,
                item::plugin,
                character::plugin,
            ))
            .configure_sets(First, (
                ServerSet::Receive,
                ServerSet::HandlePackets,
            ).chain())
            .configure_sets(PostUpdate, (
                ServerSet::UpdateVisibility,
            ))
            .configure_sets(Last, (
                ServerSet::SendFirst,
                ServerSet::SendGhosts,
                ServerSet::Send,
                ServerSet::SendLast,
            ).chain());
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
