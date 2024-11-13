use bevy::prelude::*;

pub mod cities;
pub mod maps;
pub mod skills;
pub mod locations;
pub mod static_data;
pub mod prefabs;

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            static_data::plugin,
        ));
}
