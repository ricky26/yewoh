use bevy_ecs::prelude::*;
use clap::{Parser, Subcommand};
use glam::{IVec2, IVec3};

use yewoh::protocol::OpenGump;
use yewoh_server::gump_builder::{GumpBuilder, GumpText};
use yewoh_server::world::entity::MapPosition;
use yewoh_server::world::net::{NetClient, NetOwned};

use crate::commands::{TextCommand, TextCommandQueue};

#[derive(Parser)]
pub struct GoCoordinates {
    pub map: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Subcommand)]
pub enum Command {
    Coordinates(GoCoordinates),
}

#[derive(Parser, Resource)]
pub struct Go {
    #[clap(subcommand)]
    command: Option<Command>,
}

impl TextCommand for Go {
    fn aliases() -> &'static [&'static str] {
        &["go", "teleport", "tp"]
    }
}

fn show_go_gump(client: &NetClient) {
    let size = IVec2::new(200, 300);
    let padding = IVec2::new(16, 16);
    let row = 20;

    let mut text = GumpText::new();
    let mut layout = GumpBuilder::new();
    layout
        .add_page(0)
        .add_image_sliced(0xdac, IVec2::ZERO, size)
        .add_html(
            text.intern(format!("<center>Go to Location</center>")),
            false,
            false,
            padding,
            IVec2::new(size.x - padding.x, row),
        );

    let mut y = padding.y + row * 2;
    layout
        .add_page(1)
        .add_button(0x15e1, 0x15e5, 0, 2, false, IVec2::new(padding.x, y))
        .add_html(
            text.intern(format!("<center>Place 1</center>")),
            false,
            false,
            IVec2::new(padding.x + 10, y),
            IVec2::new(size.x - padding.x * 2 - 8, row),
        );
    y += row;
    layout
        .add_page(2)
        .add_button(0x15e1, 0x15e5, 0, 1, false, IVec2::new(padding.x, y))
        .add_html(
            text.intern(format!("<center>Place 2</center>")),
            false,
            false,
            IVec2::new(padding.x + 10, y),
            IVec2::new(size.x - padding.x * 2 - 8, row),
        );
    //y += row;

    let layout = layout.into_layout(text);
    client.send_packet(OpenGump {
        id: 3,
        type_id: 2,
        position: IVec2::new(10, 10),
        layout,
    }.into());
}

pub fn go(
    clients: Query<(&NetClient, &NetOwned)>,
    mut characters: Query<&mut MapPosition>,
    mut exec: TextCommandQueue<Go>,
) {
    for (from, args) in exec.iter() {
        let (client, owned) = match clients.get(from) {
            Ok(x) => x,
            _ => continue,
        };

        let mut position = match characters.get_mut(owned.primary_entity) {
            Ok(x) => x,
            _ => continue,
        };

        match args.command {
            None => {
                show_go_gump(client);
            }
            Some(Command::Coordinates(coords)) => {
                position.map_id = coords.map;
                position.position = IVec3::new(coords.x, coords.y, coords.z);
            }
        }
    }
}
