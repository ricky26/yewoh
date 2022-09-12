use clap::Parser;

use crate::commands::{TextCommand, TextCommandQueue};

#[derive(Parser)]
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
        log::info!("echo {:?}: {}", from, cmd.what.join(" "));
    }
}