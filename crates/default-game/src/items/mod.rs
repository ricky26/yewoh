use bevy::prelude::*;

pub mod tooltips;

pub mod persistence;

#[derive(Default)]
pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((
                tooltips::plugin,
                persistence::plugin,
            ));
    }
}
