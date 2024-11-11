use bevy::prelude::*;
use clap::{Parser, Subcommand};
use glam::{IVec2, IVec3};

use yewoh::protocol::GumpLayout;
use yewoh_server::gump_builder::{GumpBuilder, GumpText};
use yewoh_server::world::entity::MapPosition;
use yewoh_server::world::connection::Possessing;
use yewoh_server::world::gump::{Gump, GumpClient};

use crate::commands::{TextCommand, TextCommandQueue, TextCommandRegistrationExt};
use crate::data::locations::{Location, LocationLevelAction, Locations};
use crate::data::static_data::StaticData;
use crate::DefaultGameSet;
use crate::entities::position::PositionExt;
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};
use crate::gumps::OnCloseGump;

#[derive(Clone, Debug)]
pub enum ButtonAction {
    Descend(String),
    GoToLocation(Location),
}

#[derive(Clone, Debug, Component)]
pub struct GoGump {
    pub character: Entity,
    pub data: Locations,
    pub prefix: String,
    pub buttons: Vec<(String, ButtonAction)>,
}

impl GoGump {
    pub fn new(character: Entity, locations: &Locations) -> GoGump {
        let mut result = GoGump {
            character,
            data: locations.clone(),
            prefix: String::new(),
            buttons: Vec::new(),
        };
        result.set_page("");
        result
    }

    pub fn set_page(&mut self, prefix: impl Into<String>) {
        self.prefix = prefix.into();
        self.buttons.clear();
        self.buttons.extend(self.data.iter_level(&self.prefix)
            .map(|(name, action)| {
                let action = match action {
                    LocationLevelAction::Descend(new_prefix) =>
                        ButtonAction::Descend(new_prefix.to_string()),
                    LocationLevelAction::Location(location) =>
                        ButtonAction::GoToLocation(location.clone()),
                };
                (name.to_string(), action)
            }));
    }

    pub fn render(&self) -> GumpLayout {
        let size = IVec2::new(400, 600);
        let padding = IVec2::new(16, 16);
        let row = 20;

        let mut text = GumpText::new();
        let mut layout = GumpBuilder::new();
        let mut page_index = 1;

        layout
            .add_image_sliced(0xdac, IVec2::ZERO, size)
            .add_html(
                text.intern("<center>Go to Location</center>".to_string()),
                false,
                false,
                padding,
                IVec2::new(size.x - padding.x, row),
            )
            .add_page(page_index);

        let prev_page_text = text.intern("<center>Previous Page</center>".to_string());
        let next_page_text = text.intern("<center>Next Page</center>".to_string());

        let mut y = padding.y + row * 2;
        if !self.prefix.is_empty() {
            layout
                .add_close_button(0x15e1, 0x15e5, 1, IVec2::new(padding.x, y))
                .add_html(
                    text.intern("<center>Back</center>".to_string()),
                    false,
                    false,
                    IVec2::new(padding.x + 10, y),
                    IVec2::new(size.x - padding.x * 2 - 8, row),
                );
            y += row;
        }

        for (button_index, (button_text, _)) in self.buttons.iter().enumerate() {
            if y >= 500 {
                page_index += 1;
                layout
                    .add_page_button(0x15e1, 0x15e5, page_index, IVec2::new(padding.x, y))
                    .add_html(
                        next_page_text,
                        false,
                        false,
                        IVec2::new(padding.x + 10, y),
                        IVec2::new(size.x - padding.x * 2 - 8, row),
                    );

                y = padding.y + row * 2;
                layout
                    .add_page(page_index)
                    .add_page_button(0x15e1, 0x15e5, page_index - 1, IVec2::new(padding.x, y))
                    .add_html(
                        prev_page_text,
                        false,
                        false,
                        IVec2::new(padding.x + 10, y),
                        IVec2::new(size.x - padding.x * 2 - 8, row),
                    );
                y += row;
            }

            layout
                .add_close_button(0x15e1, 0x15e5, button_index + 2, IVec2::new(padding.x, y))
                .add_html(
                    text.intern(format!("<center>{button_text}</center>")),
                    false,
                    false,
                    IVec2::new(padding.x + 10, y),
                    IVec2::new(size.x - padding.x * 2 - 8, row),
                );
            y += row;
        }

        layout.into_layout(text)
    }
}

pub fn handle_go_gump(
    mut commands: Commands,
    mut events: EntityEventReader<OnCloseGump, GoGump>,
    mut gumps: Query<(&mut GoGump, &mut Gump)>,
) {
    for event in events.read() {
        let Ok((mut go_gump, mut gump)) = gumps.get_mut(event.gump) else {
            continue;
        };

        if event.button_id == 0 {
            commands.entity(event.gump).despawn_recursive();
            continue;
        }

        if event.button_id == 1 {
            // Go up a level
            if let Some(new_len) = go_gump.prefix.trim_end_matches('/').rfind('/') {
                let new_prefix = go_gump.prefix[..(new_len + 1)].to_string();
                go_gump.set_page(new_prefix);
            } else {
                go_gump.set_page("");
            };
            gump.set_layout(go_gump.render());
            continue;
        }

        let action_index = (event.button_id - 2) as usize;
        let Some((_, action)) = go_gump.buttons.get(action_index) else {
            continue;
        };

        match action {
            ButtonAction::Descend(new_prefix) => {
                let new_prefix = new_prefix.clone();
                go_gump.set_page(new_prefix);
                gump.set_layout(go_gump.render());
            }
            ButtonAction::GoToLocation(location) => {
                commands.entity(event.gump).despawn_recursive();
                let character = go_gump.character;
                commands.entity(character)
                    .move_to_map_position(MapPosition {
                        position: location.position,
                        map_id: location.map_id as u8,
                        ..default()
                    });
            }
        }
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
    static_data: Res<StaticData>,
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
                let go_gump = GoGump::new(owned.entity, &static_data.locations);
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
        .add_text_command::<Go>()
        .add_systems(Update, (
            go,
        ))
        .add_systems(First, (
            handle_go_gump.in_set(DefaultGameSet::HandleEvents),
        ));
}
