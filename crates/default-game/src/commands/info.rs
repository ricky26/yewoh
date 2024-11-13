use std::fmt::Write;
use std::sync::Arc;
use bevy::ecs::component::ComponentId;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy::reflect::ReflectRef;
use clap::Parser;
use glam::ivec2;
use yewoh::assets::map::CHUNK_SIZE;
use yewoh::protocol::{GumpLayout, TargetType};
use yewoh_server::gump_builder::{GumpBoxLayout, GumpBuilder, GumpRect, GumpRectLayout, GumpText};
use yewoh_server::world::gump::{Gump, GumpClient};
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse, WorldTargetRequest, WorldTargetResponse};
use yewoh_server::world::spatial::SpatialQuery;
use yewoh_server::world::view::ViewKey;

use crate::commands::{TextCommand, TextCommandQueue, TextCommandRegistrationExt};
use crate::DefaultGameSet;
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};
use crate::gumps::{OnCloseGump, RESIZABLE_PAPER_3};
use crate::gumps::page_allocator::GumpPageBoxAllocator;

const ROW_HEIGHT: i32 = 20;

fn type_short_name(name: &str) -> &str {
    let max = name.rfind('<').unwrap_or(name.len());
    if let Some(n) = name[..max].rfind(':') {
        &name[(n + 1)..]
    } else {
        name
    }
}

fn debug_value(value: &dyn Reflect) -> String {
    match value.reflect_ref() {
        ReflectRef::Struct(reflect) => {
            if reflect.field_len() == 0 {
                String::new()
            } else {
                let mut result = String::new();

                for (index, field) in reflect.iter_fields().enumerate() {
                    if !result.is_empty() {
                        result.push_str(", ");
                    }

                    if let Some(name) = reflect.name_at(index) {
                        write!(&mut result, "{name}: {field:?}").ok();
                    } else {
                        write!(&mut result, "{index}: {field:?}").ok();
                    }
                }

                result
            }
        }
        ReflectRef::TupleStruct(reflect) => {
            let mut result = String::new();

            for element in reflect.iter_fields() {
                if !result.is_empty() {
                    result.push_str(", ");
                }

                write!(&mut result, "{element:?}").ok();
            }

            result
        }
        ReflectRef::Tuple(reflect) => {
            let mut result = String::new();

            for element in reflect.iter_fields() {
                if !result.is_empty() {
                    result.push_str(", ");
                }

                write!(&mut result, "{element:?}").ok();
            }

            result
        }
        ReflectRef::List(reflect) => {
            let mut result = String::new();

            for element in reflect.iter() {
                if result.len() > 1 {
                    result.push_str(", ");
                }

                write!(&mut result, "{element:?}").ok();
            }

            result
        }
        ReflectRef::Array(reflect) => {
            let mut result = String::new();

            for element in reflect.iter() {
                if !result.is_empty() {
                    result.push_str(", ");
                }

                write!(&mut result, "{element:?}").ok();
            }

            result
        }
        ReflectRef::Map(_) => "{..}".to_string(),
        ReflectRef::Set(_) => "(..)".to_string(),
        _ => format!("{value:?}"),
    }
}

#[derive(Clone, Copy, Debug)]
pub enum InfoGumpPage {
    Chunk(u8, IVec2),
    Entity(Entity),
    Component(Entity, ComponentId),
}

#[derive(Component)]
pub struct InfoGump {
    chunk: Option<(u8, IVec2)>,
    page: InfoGumpPage,
    spatial_query: SystemState<SpatialQuery<'static>>,
    entities: Vec<Entity>,
    components: Vec<(String, ComponentId, String, bool)>,
    component_info: Option<(String, Arc<dyn PartialReflect>)>,
}

impl InfoGump {
    pub fn new(world: &mut World, page: InfoGumpPage) -> InfoGump {
        let chunk = if let InfoGumpPage::Chunk(map_id, chunk) = page {
            Some((map_id, chunk))
        } else {
            None
        };
        let spatial_query = SystemState::new(world);
        let mut gump = InfoGump {
            chunk,
            page,
            spatial_query,
            entities: Vec::new(),
            components: Vec::new(),
            component_info: None,
        };
        gump.set_page(world, gump.page);
        gump
    }

    pub fn set_page(&mut self, world: &World, page: InfoGumpPage) {
        self.page = page;
        self.components.clear();

        match &self.page {
            InfoGumpPage::Chunk(map_id, chunk) => {
                let spatial_query = self.spatial_query.get(world);
                let min = *chunk * CHUNK_SIZE as i32;
                let max = min + CHUNK_SIZE as i32;

                for y in min.y..max.y {
                    for x in min.x..max.x {
                        let pos = ivec2(x, y);
                        self.entities.extend(spatial_query.iter_at(*map_id, pos));
                    }
                }
            }
            InfoGumpPage::Entity(entity) => {
                let type_registry = world.resource::<AppTypeRegistry>().clone();
                let type_registry = type_registry.read();

                if let Ok(entity) = world.get_entity(*entity) {
                    let component_types = world.components();
                    let archetype = entity.archetype();

                    for component_type in archetype.components() {
                        let component_info = component_types.get_info(component_type).unwrap();
                        let reflected = component_info.type_id()
                            .and_then(|id| type_registry.get(id))
                            .and_then(|r| r.data::<ReflectComponent>())
                            .and_then(|r| r.reflect(entity));
                        let can_navigate = reflected.is_some();
                        let name = type_short_name(component_info.name()).to_string();
                        let value = reflected.map(debug_value)
                            .unwrap_or_else(|| "".to_string());
                        self.components.push((name, component_type, value, can_navigate));
                    }
                }
            }
            InfoGumpPage::Component(entity, component_type) => {
                if let Ok(entity) = world.get_entity(*entity) {
                    let type_registry = world.resource::<AppTypeRegistry>().clone();
                    let type_registry = type_registry.read();
                    let component_types = world.components();
                    let component_info = component_types.get_info(*component_type).unwrap();
                    let reflected = component_info.type_id()
                        .and_then(|id| type_registry.get(id))
                        .and_then(|r| r.data::<ReflectComponent>())
                        .and_then(|r| r.reflect(entity))
                        .map_or_else::<Arc<dyn PartialReflect>, _, _>(
                            || Arc::new(()),
                            |v| Arc::from(v.clone_value()));
                    let name = type_short_name(component_info.name()).to_string();
                    self.component_info = Some((name, reflected));
                }
            }
        }
    }

    fn render_chunk(
        &self,
        mut layout: GumpBoxLayout,
        map_id: u8,
        chunk: IVec2,
    ) {
        layout
            .allocate(ROW_HEIGHT, |builder| builder
                .html(format!("<center>Chunk {map_id} - {chunk:?}</center>")))
            .gap(ROW_HEIGHT);

        let mut page = GumpPageBoxAllocator::new(layout.rest(), 1);
        for (entity_index, entity) in self.entities.iter().enumerate() {
            page.allocate(ROW_HEIGHT, |builder| builder
                .background(|b| b.html(format!("<center>{entity}</center>")))
                .right(16)
                .close_button(0x15e1, 0x15e5, entity_index + 1));
        }
    }

    fn render_entity(
        &self,
        mut layout: GumpBoxLayout,
        entity: Entity,
    ) {
        layout
            .allocate(ROW_HEIGHT, |builder| builder
                .html(format!("<center>Entity {entity}</center>")))
            .gap(ROW_HEIGHT);

        if self.chunk.is_some() {
            layout
                .allocate(ROW_HEIGHT, |builder| builder
                    .background(|builder| builder
                        .html("<center>Back</center>"))
                    .right(16)
                    .close_button(0x15e3, 0x15e7, 1))
                .gap(ROW_HEIGHT);
        }

        let mut page = GumpPageBoxAllocator::new(layout.rest(), 1);
        for (component_index, (type_name, _, value, can_navigate)) in self.components.iter().enumerate() {
            page.allocate(ROW_HEIGHT * 3, |builder| {
                builder
                    .into_vbox()
                    .allocate(ROW_HEIGHT, |builder| builder
                        .html(format!("<center>{type_name}</center>")))
                    .allocate(ROW_HEIGHT, |builder| {
                        let builder = builder
                            .background(|builder| builder
                                .html(format!("<center>{value}</center>")));

                        if *can_navigate {
                            builder
                                .right(16)
                                .close_button(0x15e1, 0x15e5, component_index + 2);
                        }
                    });
            });
        }
    }

    fn render_component(
        &self,
        mut layout: GumpBoxLayout,
        _entity: Entity,
        _component_type: ComponentId,
    ) {
        let (type_name, value) = self.component_info.as_ref().unwrap();
        layout
            .allocate(ROW_HEIGHT, |builder| builder
                .html(format!("<center>{type_name}</center>")))
            .allocate(ROW_HEIGHT, |builder| builder
                .background(|builder| builder
                    .html("<center>Back</center>"))
                .right(16)
                .close_button(0x15e3, 0x15e7, 1))
            .allocate(ROW_HEIGHT, |builder| builder
                .html(format!("<center>{value:#?}</center>")));
    }

    pub fn render(&self) -> GumpLayout {
        let mut text = GumpText::new();
        let mut layout = GumpBuilder::new();

        let rect = GumpRect::from_zero(ivec2(400, 600));
        let box_layout = GumpRectLayout::new(&mut layout, &mut text, rect)
            .background(|builder| builder
                .image_sliced(RESIZABLE_PAPER_3))
            .with_padding(16)
            .into_vbox();

        match self.page {
            InfoGumpPage::Chunk(map_id, chunk) =>
                self.render_chunk(box_layout, map_id, chunk),
            InfoGumpPage::Entity(entity) =>
                self.render_entity(box_layout, entity),
            InfoGumpPage::Component(entity, component) =>
                self.render_component(box_layout, entity, component),
        }

        layout.into_layout(text)
    }
}

pub fn handle_info_gump(
    mut commands: Commands,
    mut events: EntityEventReader<OnCloseGump, InfoGump>,
    mut gumps: Query<&InfoGump>,
) {
    for event in events.read() {
        let Ok(info_gump) = gumps.get_mut(event.gump) else {
            continue;
        };

        if event.button_id == 0 {
            commands.entity(event.gump).despawn_recursive();
            continue;
        }

        let next_page = match info_gump.page {
            InfoGumpPage::Chunk(_, _) => {
                let Some(entity) = info_gump.entities.get((event.button_id - 1) as usize) else {
                    continue;
                };
                InfoGumpPage::Entity(*entity)
            }
            InfoGumpPage::Entity(entity) => {
                if event.button_id == 1 {
                    let (map_id, chunk) = info_gump.chunk.unwrap();
                    InfoGumpPage::Chunk(map_id, chunk)
                } else {
                    let Some((_, component_type, ..)) = info_gump.components.get((event.button_id - 2) as usize) else {
                        continue;
                    };
                    InfoGumpPage::Component(entity, *component_type)
                }
            }
            InfoGumpPage::Component(entity, _) => {
                InfoGumpPage::Entity(entity)
            }
        };

        let gump_entity = event.gump;
        commands.queue(move |world: &mut World| {
            let Some((mut info_gump, mut gump)) = world.entity_mut(gump_entity).take::<(InfoGump, Gump)>() else {
                return;
            };
            info_gump.set_page(world, next_page);
            gump.set_layout(info_gump.render());
            world.entity_mut(gump_entity)
                .insert((info_gump, gump));
        });
    }
}

#[derive(Parser, Resource)]
pub struct EntityInfo;

impl TextCommand for EntityInfo {
    fn aliases() -> &'static [&'static str] {
        &["info"]
    }
}

#[derive(Parser, Resource)]
pub struct ChunkInfo;

impl TextCommand for ChunkInfo {
    fn aliases() -> &'static [&'static str] {
        &["chunkinfo", "infoat"]
    }
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct ShowInfoCommand;

pub fn start_info(
    mut exec: TextCommandQueue<EntityInfo>,
    mut exec_tile: TextCommandQueue<ChunkInfo>,
    mut commands: Commands,
) {
    for (from, _) in exec.iter() {
        commands.spawn((
            EntityTargetRequest {
                client_entity: from,
                target_type: TargetType::Neutral,
            },
            ShowInfoCommand));
    }

    for (from, _) in exec_tile.iter() {
        commands.spawn((
            WorldTargetRequest {
                client_entity: from,
                target_type: TargetType::Neutral,
            },
            ShowInfoCommand,
        ));
    }
}

pub fn info(
    clients: Query<&ViewKey>,
    completed_tile: Query<(Entity, &WorldTargetRequest, &WorldTargetResponse), With<ShowInfoCommand>>,
    completed_entity: Query<(Entity, &EntityTargetRequest, &EntityTargetResponse), With<ShowInfoCommand>>,
    mut commands: Commands,
) {
    for (entity, request, response) in completed_tile.iter() {
        commands.entity(entity).despawn();

        let Ok(view_key) = clients.get(request.client_entity) else {
            continue;
        };

        let Some(position) = response.position else {
            continue;
        };

        let client_entity = request.client_entity;
        let map_id = view_key.map_id;
        let chunk = position.truncate() / (CHUNK_SIZE as i32);
        commands.queue(move |world: &mut World| {
            let mut gump = Gump::empty(1235);
            let go_gump = InfoGump::new(world, InfoGumpPage::Chunk(map_id, chunk));
            let layout = go_gump.render();
            gump.set_layout(layout);
            world
                .spawn((
                    gump,
                    GumpClient(client_entity),
                    go_gump,
                ));
        });
    }

    for (entity, request, response) in completed_entity.iter() {
        commands.entity(entity).despawn();

        if let Some(target) = response.target {
            let client_entity = request.client_entity;
            commands.queue(move |world: &mut World| {
                let mut gump = Gump::empty(1235);
                let go_gump = InfoGump::new(world, InfoGumpPage::Entity(target));
                let layout = go_gump.render();
                gump.set_layout(layout);
                world
                    .spawn((
                        gump,
                        GumpClient(client_entity),
                        go_gump,
                    ));
            });
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventRoutePlugin::<OnCloseGump, InfoGump>::default(),
        ))
        .add_text_command::<EntityInfo>()
        .add_text_command::<ChunkInfo>()
        .add_systems(Update, (
            start_info,
            info,
        ))
        .add_systems(First, (
            handle_info_gump.in_set(DefaultGameSet::HandleEvents),
        ));
}
