use bevy::prelude::*;
use clap::Parser;
use glam::IVec2;
use tracing::info;
use yewoh::protocol::{GumpLayout, OpenGump};
use yewoh::Direction;
use yewoh_server::world::connection::{NetClient, Possessing};
use yewoh_server::world::entity::{Graphic, Hue, MapPosition};

use crate::commands::{TextCommand, TextCommandQueue};

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

#[derive(Parser, Resource)]
pub struct FryPan;

impl TextCommand for FryPan {
    fn aliases() -> &'static [&'static str] {
        &["frypan"]
    }
}

pub fn frypan(
    mut exec: TextCommandQueue<FryPan>,
    owners: Query<&Possessing>,
    characters: Query<&MapPosition>,
    mut commands: Commands,
) {
    for (from, _) in exec.iter() {
        if let Some(position) = owners.get(from)
            .ok()
            .and_then(|owner| characters.get(owner.entity).ok()) {
            commands.spawn((
                MapPosition {
                    map_id: 1,
                    position: position.position,
                    direction: Direction::North,
                },
                Graphic(0x97f),
                Hue(0x7d0),
            ));
        }
    }
}

#[derive(Parser, Resource)]
pub struct TestGump;

impl TextCommand for TestGump {
    fn aliases() -> &'static [&'static str] {
        &["testgump"]
    }
}

pub fn test_gump(
    mut exec: TextCommandQueue<TestGump>,
    clients: Query<&NetClient>,
) {
    for (from, _) in exec.iter() {
        if let Ok(client) = clients.get(from) {
            client.send_packet(OpenGump {
                id: 1,
                type_id: 2,
                position: IVec2::new(50, 50),
                layout: GumpLayout {
                    layout: "{ page 0 }{ resizepic 0 0 5054 420 440 }{ text 0 0 120 0 }".to_string(),
                    text: vec!["Hello, world!".into()],
                },
            }.into());
        }
    }
}
