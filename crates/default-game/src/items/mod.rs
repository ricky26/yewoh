use bevy::prelude::*;

pub mod persistence;

pub mod common;

pub mod containers;

pub const MAX_STACK: u16 = 60000;

#[derive(Default)]
pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((
                persistence::plugin,
                common::plugin,
                containers::plugin,
            ));
    }
}
