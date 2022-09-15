use bevy_app::prelude::*;

use crate::accounts::{handle_create_character, handle_list_characters, handle_list_characters_callback, handle_select_character, handle_spawn_character, PendingCharacterInfo, PendingCharacterLists};
use crate::accounts::repository::MemoryAccountRepository;
use crate::actions::{handle_context_menu, handle_double_click, handle_drop, handle_equip, handle_move, handle_pick_up, handle_profile_requests, handle_single_click, handle_skills_requests, handle_war_mode};
use crate::chat::handle_incoming_chat;
use crate::commands::TextCommands;
use crate::npc::{init_npcs, spawn_npcs};

pub mod accounts;

pub mod data;

pub mod actions;

pub mod chat;

pub mod commands;

pub mod npc;

#[derive(Default)]
pub struct DefaultGamePlugin;

impl Plugin for DefaultGamePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MemoryAccountRepository>()
            .init_resource::<PendingCharacterLists>()
            .init_resource::<PendingCharacterInfo>()
            .insert_resource(TextCommands::new('['))
            .add_startup_system(init_npcs)
            .add_system(spawn_npcs)
            .add_system(handle_list_characters::<MemoryAccountRepository>)
            .add_system(handle_list_characters_callback)
            .add_system(handle_create_character::<MemoryAccountRepository>)
            .add_system(handle_select_character::<MemoryAccountRepository>)
            .add_system(handle_spawn_character)
            .add_system(handle_move)
            .add_system(handle_single_click)
            .add_system(handle_double_click)
            .add_system(handle_pick_up)
            .add_system(handle_drop)
            .add_system(handle_equip)
            .add_system(handle_war_mode)
            .add_system(handle_incoming_chat)
            .add_system(handle_context_menu)
            .add_system(handle_profile_requests)
            .add_system(handle_skills_requests)
            .add_system(commands::info::info)
            .add_system(commands::test::echo)
            .add_system(commands::test::frypan)
            .add_system(commands::test::test_gump);
    }

    fn name(&self) -> &str { "Yewoh Default Game" }
}
