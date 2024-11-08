use bevy::prelude::*;
use yewoh::Direction;

pub mod player;

pub mod persistence;

#[derive(Clone, Debug, Default, Event)]
pub struct OnCharacterMove {
    pub blocked: bool,
    pub direction: Direction,
    pub run: bool,
}

pub fn plugin(app: &mut App) {
    app
        .add_event::<OnCharacterMove>()
        .add_plugins((
            player::plugin,
            persistence::plugin,
        ));
}
