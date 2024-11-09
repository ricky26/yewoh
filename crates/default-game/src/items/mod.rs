use bevy::prelude::*;

pub mod persistence;

pub mod containers;

#[derive(Default)]
pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((
                persistence::plugin,
                containers::plugin,
            ));
    }
}
