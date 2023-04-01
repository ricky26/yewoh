use bevy_app::PluginGroupBuilder;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

use yewoh_server::world::ServerSet;

use crate::accounts::AccountsPlugin;
use crate::actions::{handle_context_menu, handle_double_click, handle_drop, handle_equip, handle_move, handle_pick_up, handle_profile_requests, handle_single_click, handle_skills_requests, handle_war_mode};
use crate::activities::ActivitiesPlugin;
use crate::characters::CharactersPlugin;
use crate::chat::handle_incoming_chat;
use crate::commands::CommandsPlugin;
use crate::data::prefab::PrefabPlugin;
use crate::npc::{init_npcs, move_npcs, spawn_npcs};
use crate::time::send_time;

pub mod accounts;

pub mod data;

pub mod actions;

pub mod chat;

pub mod commands;

pub mod npc;

pub mod time;

pub mod activities;

pub mod characters;

#[derive(Default)]
pub struct DefaultGamePlugins;

impl PluginGroup for DefaultGamePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PrefabPlugin)
            .add(CommandsPlugin)
            .add(AccountsPlugin)
            .add(DefaultGamePlugin)
            .add(ActivitiesPlugin)
            .add(CharactersPlugin)
    }
}

#[derive(Default)]
pub struct DefaultGamePlugin;

impl Plugin for DefaultGamePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_startup_system(init_npcs)
            .add_systems((
                spawn_npcs,
                move_npcs,
            ).in_base_set(CoreSet::Update))
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
