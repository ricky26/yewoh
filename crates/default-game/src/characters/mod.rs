use bevy::prelude::*;

pub mod player;

pub mod persistence;

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            player::plugin,
            persistence::plugin,
        ));
}
