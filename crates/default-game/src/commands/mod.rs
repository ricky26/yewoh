use bevy::prelude::*;

pub use registration::{
    TextCommand,
    TextCommandExecutor,
    TextCommandQueue,
    TextCommandRegistrationExt,
    TextCommands,
};

mod registration;

pub mod test;

pub mod info;

pub mod go;

pub mod spawn;

pub mod destroy;

pub struct CommandsPlugin;

impl Plugin for CommandsPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(TextCommands::new('['))
            .add_plugins((
                spawn::plugin,
                destroy::plugin,
                info::plugin,
                go::plugin,
                test::plugin,
            ));
    }
}
