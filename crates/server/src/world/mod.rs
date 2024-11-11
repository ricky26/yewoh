use bevy::prelude::*;

pub mod entity;

pub mod characters;

pub mod items;

pub mod chat;

pub mod spatial;

pub mod delta_grid;

pub mod navigation;

pub mod map;

pub mod input;

pub mod combat;

pub mod net_id;

pub mod connection;

pub mod view;

pub mod account;

pub mod gump;

#[derive(SystemSet, Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerSet {
    Receive,
    HandlePackets,
    UpdateVisibility,
    AssignNetIds,
    SendFirst,
    QueueDeltas,
    DestroyEntities,
    DetectChanges,
    SendEntities,
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
                chat::plugin,
                view::plugin,
                input::plugin,
                combat::plugin,
                delta_grid::plugin,
                items::plugin,
                characters::plugin,
                account::plugin,
                gump::plugin,
            ))
            .configure_sets(First, (
                (
                    ServerSet::Receive,
                    ServerSet::HandlePackets,
                ).chain(),
            ).chain())
            .configure_sets(PostUpdate, (
                ServerSet::UpdateVisibility,
                ServerSet::AssignNetIds,
            ))
            .configure_sets(Last, (
                ServerSet::SendFirst,
                ServerSet::DetectChanges,
                ServerSet::SendEntities,
                ServerSet::Send,
                ServerSet::SendLast,
            ).chain());
    }

    fn name(&self) -> &str { "Yewoh Server" }
}
