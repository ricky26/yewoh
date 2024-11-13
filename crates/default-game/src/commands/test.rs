use bevy::prelude::*;
use clap::Parser;
use tracing::info;

use crate::commands::{TextCommand, TextCommandQueue, TextCommandRegistrationExt};

#[derive(Parser, Resource)]
pub struct Echo {
    pub what: Vec<String>,
}

impl TextCommand for Echo {
    fn aliases() -> &'static [&'static str] {
        &["echo"]
    }
}

pub fn echo(mut exec: TextCommandQueue<Echo>) {
    for (from, cmd) in exec.iter() {
        info!("echo {:?}: {}", from, cmd.what.join(" "));
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_text_command::<Echo>()
        .add_systems(Update, (
            echo,
        ));
}

