use bevy::prelude::*;
use clap::{Parser, Subcommand};
use glam::{IVec2, IVec3};

use yewoh::protocol::GumpLayout;
use yewoh_server::gump_builder::{GumpBuilder, GumpText};
use yewoh_server::world::entity::MapPosition;
use yewoh_server::world::connection::Possessing;
use yewoh_server::world::gump::{Gump, GumpClient};

use crate::commands::{TextCommand, TextCommandQueue, TextCommandRegistrationExt};
use crate::DefaultGameSet;
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};
use crate::gumps::OnCloseGump;

#[derive(Clone, Debug, Component, Reflect)]
#[reflect(Component)]
pub struct GoGump {
}

impl GoGump {
    pub fn render(&self) -> GumpLayout {
        let size = IVec2::new(200, 300);
        let padding = IVec2::new(16, 16);
        let row = 20;

        let mut text = GumpText::new();
        let mut layout = GumpBuilder::new();
        layout
            .add_page(0)
            .add_image_sliced(0xdac, IVec2::ZERO, size)
            .add_html(
                text.intern("<center>Go to Location</center>".to_string()),
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
                text.intern("<center>Place 1</center>".to_string()),
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
                text.intern("<center>Place 2</center>".to_string()),
                false,
                false,
                IVec2::new(padding.x + 10, y),
                IVec2::new(size.x - padding.x * 2 - 8, row),
            );
        //y += row;

        layout.into_layout(text)
    }
}

pub fn handle_go_gump(
    mut commands: Commands,
    mut events: EntityEventReader<OnCloseGump, GoGump>,
) {
    for event in events.read() {
        commands.entity(event.gump).despawn_recursive();
        warn!("go gump response {event:?}");
    }
}

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

pub fn go(
    mut commands: Commands,
    clients: Query<&Possessing>,
    mut characters: Query<&mut MapPosition>,
    mut exec: TextCommandQueue<Go>,
) {
    for (from, args) in exec.iter() {
        let Ok(owned) = clients.get(from) else {
            continue;
        };

        let Ok(mut position) = characters.get_mut(owned.entity) else {
            continue;
        };

        match args.command {
            None => {
                let mut gump = Gump::empty(1234);
                let go_gump = GoGump{};
                let layout = go_gump.render();
                gump.set_layout(layout);

                commands.spawn((
                    gump,
                    GumpClient(from),
                    go_gump,
                ));
            }
            Some(Command::Coordinates(coords)) => {
                position.map_id = coords.map;
                position.position = IVec3::new(coords.x, coords.y, coords.z);
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventRoutePlugin::<OnCloseGump, GoGump>::default(),
        ))
        .register_type::<GoGump>()
        .add_text_command::<Go>()
        .add_systems(Update, (
            go,
        ))
        .add_systems(First, (
            handle_go_gump.in_set(DefaultGameSet::HandleEvents),
        ));
}
