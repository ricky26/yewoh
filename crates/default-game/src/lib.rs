#![allow(clippy::type_complexity)]

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy_fabricator::FabricatorPlugin;
use yewoh_server::world::ServerSet;

use crate::accounts::AccountsPlugin;
use crate::activities::ActivitiesPlugin;
use crate::ai::AiPlugin;
use crate::chat::on_client_chat_message;
use crate::commands::CommandsPlugin;
use crate::entities::EntitiesPlugin;
use crate::items::ItemsPlugin;
use crate::persistence::PersistencePlugin;
use crate::spawners::SpawnersPlugin;
use crate::time::send_time;

pub mod entity_events;

pub mod accounts;

pub mod data;

pub mod actions;

pub mod chat;

pub mod commands;

pub mod spawners;

pub mod time;

pub mod activities;

pub mod characters;

pub mod items;

pub mod ai;

pub mod persistence;

pub mod networking;

pub mod hues;

pub mod entities;

pub mod format;

pub mod l10n;

#[derive(Clone, Debug, Hash, PartialEq, Eq, SystemSet)]
pub enum DefaultGameSet {
    DispatchEvents,
    HandleEvents,
    FinishEvents,
}

#[derive(Default)]
pub struct DefaultGamePlugins;

impl PluginGroup for DefaultGamePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(FabricatorPlugin)
            .add(PersistencePlugin)
            .add(CommandsPlugin)
            .add(AccountsPlugin::<accounts::sql::SqlAccountRepository>::default())
            .add(DefaultGamePlugin)
            .add(ActivitiesPlugin)
            .add(EntitiesPlugin)
            .add(characters::plugin)
            .add(ItemsPlugin)
            .add(SpawnersPlugin)
            .add(AiPlugin)
    }
}

#[derive(Default)]
pub struct DefaultGamePlugin;

impl Plugin for DefaultGamePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((
                actions::plugin,
                l10n::plugin,
            ))
            .configure_sets(First, (
                (
                    DefaultGameSet::DispatchEvents.after(ServerSet::HandlePackets),
                    DefaultGameSet::HandleEvents,
                    DefaultGameSet::FinishEvents,
                ).chain(),
            ))
            .add_systems(First, (
                on_client_chat_message.in_set(ServerSet::HandlePackets),
            ))
            .add_systems(Last, (
                send_time.in_set(ServerSet::Send),
            ));
    }

    fn name(&self) -> &str { "Yewoh Default Game" }
}
