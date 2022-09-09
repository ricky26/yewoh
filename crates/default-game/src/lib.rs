use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

use yewoh_server::world::client::apply_new_primary_entities;

use crate::accounts::{handle_create_character, handle_list_characters};
use crate::actions::handle_move;
use crate::space::{Space, update_space};

pub mod space;

pub mod accounts;

pub mod data;

pub mod actions;

#[derive(Default)]
pub struct DefaultGamePlugin;

impl Plugin for DefaultGamePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<Space>()
            .add_system(handle_list_characters)
            .add_system(handle_create_character.after(apply_new_primary_entities))
            .add_system(handle_move)
            .add_system_to_stage(CoreStage::Last, update_space);
    }

    fn name(&self) -> &str { "Yewoh Default Game" }
}
