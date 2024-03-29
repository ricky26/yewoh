use bevy_app::{App, CoreSet, Plugin};
use bevy_ecs::schedule::IntoSystemConfigs;
pub use registration::{
    TextCommands,
    TextCommand,
    TextCommandExecutor,
    TextCommandQueue,
    TextCommandRegistrationExt,
};

mod registration;

pub mod test;

pub mod info;

pub mod go;

pub mod spawn;

pub struct CommandsPlugin;

impl Plugin for CommandsPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(TextCommands::new('['))
            .add_text_command::<go::Go>()
            .add_text_command::<info::Info>()
            .add_text_command::<info::TileInfo>()
            .add_text_command::<test::Echo>()
            .add_text_command::<test::FryPan>()
            .add_text_command::<test::TestGump>()
            .add_text_command::<spawn::Spawn>()
            .add_systems((
                info::info,
                info::start_info,
                go::go,
                test::echo,
                test::frypan,
                test::test_gump,
                spawn::start_spawn,
                spawn::spawn,
            ).in_base_set(CoreSet::Update));
    }
}
