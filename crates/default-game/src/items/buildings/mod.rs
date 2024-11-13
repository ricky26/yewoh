use bevy::prelude::*;

pub mod doors;

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            doors::plugin,
        ));
}
