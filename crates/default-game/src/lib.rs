use bevy_app::PluginGroupBuilder;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

use yewoh_server::world::ServerSet;

use crate::accounts::AccountsPlugin;
use crate::actions::{handle_context_menu, handle_double_click, handle_drop, handle_equip, handle_move, handle_pick_up, handle_profile_requests, handle_single_click, handle_skills_requests, handle_war_mode};
use crate::activities::ActivitiesPlugin;
use crate::ai::AiPlugin;
use crate::characters::CharactersPlugin;
use crate::chat::handle_incoming_chat;
use crate::commands::CommandsPlugin;
use crate::data::prefab::PrefabPlugin;
use crate::entities::EntitiesPlugin;
use crate::items::ItemsPlugin;
use crate::persistence::PersistencePlugin;
use crate::spawners::SpawnersPlugin;
use crate::time::send_time;

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

#[derive(Default)]
pub struct DefaultGamePlugins;

impl PluginGroup for DefaultGamePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PrefabPlugin)
            .add(PersistencePlugin)
            .add(CommandsPlugin)
            .add(AccountsPlugin::<accounts::sql::SqlAccountRepository>::default())
            .add(DefaultGamePlugin)
            .add(ActivitiesPlugin)
            .add(EntitiesPlugin)
            .add(CharactersPlugin)
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
            .add_systems((
                handle_move,
                handle_single_click,
                handle_double_click,
                handle_pick_up,
                handle_drop,
                handle_equip,
                handle_war_mode,
                handle_incoming_chat,
                handle_context_menu,
                handle_profile_requests,
                handle_skills_requests,
            ).in_base_set(CoreSet::Update))
            .add_system(send_time.in_set(ServerSet::Send));
    }

    fn name(&self) -> &str { "Yewoh Default Game" }
}
