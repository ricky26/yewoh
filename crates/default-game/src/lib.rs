use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

use yewoh_server::world::ServerSet;

use crate::accounts::{handle_create_character, handle_list_characters, handle_list_characters_callback, handle_select_character, handle_spawn_character, PendingCharacterInfo, PendingCharacterLists};
use crate::accounts::repository::MemoryAccountRepository;
use crate::actions::{handle_context_menu, handle_double_click, handle_drop, handle_equip, handle_move, handle_pick_up, handle_profile_requests, handle_single_click, handle_skills_requests, handle_war_mode};
use crate::chat::handle_incoming_chat;
use crate::commands::{TextCommandRegistrationExt, TextCommands};
use crate::commands::go::Go;
use crate::commands::info::Info;
use crate::commands::test::{Echo, FryPan, TestGump};
use crate::npc::{init_npcs, spawn_npcs};
use crate::time::send_time;

pub mod accounts;

pub mod data;

pub mod actions;

pub mod chat;

pub mod commands;

pub mod npc;

pub mod time;

#[derive(Default)]
pub struct DefaultGamePlugin;

impl Plugin for DefaultGamePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MemoryAccountRepository>()
            .init_resource::<PendingCharacterLists>()
            .init_resource::<PendingCharacterInfo>()
            .insert_resource(TextCommands::new('['))
            .add_text_command::<Go>()
            .add_text_command::<Info>()
            .add_text_command::<Echo>()
            .add_text_command::<FryPan>()
            .add_text_command::<TestGump>()
            .add_startup_system(init_npcs)
            .add_systems((
                handle_list_characters::<MemoryAccountRepository>,
                handle_list_characters_callback,
                handle_create_character::<MemoryAccountRepository>,
                handle_select_character::<MemoryAccountRepository>,
                handle_spawn_character,
            ).in_base_set(CoreSet::Update))
            .add_systems((
                spawn_npcs,
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
            .add_system(send_time.in_set(ServerSet::Send))
            .add_systems((
                commands::info::info,
                commands::go::go,
                commands::test::echo,
                commands::test::frypan,
                commands::test::test_gump,
            ).in_base_set(CoreSet::Update));
    }

    fn name(&self) -> &str { "Yewoh Default Game" }
}
