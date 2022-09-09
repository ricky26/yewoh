use bevy_app::prelude::*;

use crate::accounts::{handle_create_character, handle_list_characters};
use crate::actions::{handle_double_click, handle_move};
use crate::chat::handle_incoming_chat;
use crate::space::{Space, update_space};

pub mod space;

pub mod accounts;

pub mod data;

pub mod actions;

pub mod chat;

#[derive(Default)]
pub struct DefaultGamePlugin;

impl Plugin for DefaultGamePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<Space>()
            .add_system(handle_list_characters)
            .add_system(handle_create_character)
            .add_system(handle_move)
            .add_system(handle_double_click)
            .add_system(handle_incoming_chat)
            .add_system_to_stage(CoreStage::Last, update_space);
    }

    fn name(&self) -> &str { "Yewoh Default Game" }
}
