use bevy_app::AppTypeRegistry;
use bevy_ecs::archetype::Archetypes;
use bevy_ecs::component::Components;
use bevy_ecs::entity::Entities;
use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use clap::Parser;

use yewoh::protocol::{MessageKind, TargetType, UnicodeTextMessage};
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse, WorldTargetRequest, WorldTargetResponse};
use yewoh_server::world::net::{NetClient, ViewState};
use yewoh_server::world::spatial::EntityPositions;

use crate::commands::{TextCommand, TextCommandQueue};

#[derive(Parser, Resource)]
pub struct Info;

impl TextCommand for Info {
    fn aliases() -> &'static [&'static str] {
        &["info"]
    }
}

#[derive(Parser, Resource)]
pub struct TileInfo;

impl TextCommand for TileInfo {
    fn aliases() -> &'static [&'static str] {
        &["tileinfo"]
    }
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct ShowInfoCommand;

pub fn info(
    archetypes: &Archetypes,
    components: &Components,
    entities: &Entities,
    clients: Query<&NetClient>,
    completed: Query<(Entity, &EntityTargetRequest, &EntityTargetResponse), With<ShowInfoCommand>>,
    mut exec: TextCommandQueue<Info>,
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

    for (entity, request, response) in completed.iter() {
        commands.entity(entity).despawn();

        let client = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        if let Some(target) = response.target {
            client.send_packet(UnicodeTextMessage {
                kind: MessageKind::System,
                text: "Picked Target".to_string(),
                hue: 120,
                font: 3,
                ..Default::default()
            }.into());

            if let Some(archetype) = entities.get(target)
                .and_then(|l| archetypes.get(l.archetype_id)) {
                for component in archetype.components() {
                    if let Some(info) = components.get_info(component) {
                        client.send_packet(UnicodeTextMessage {
                            kind: MessageKind::System,
                            text: format!("Has Component {}", info.name()),
                            hue: 120,
                            font: 3,
                            ..Default::default()
                        }.into());
                    }
                }
            }
        } else {
            client.send_packet(UnicodeTextMessage {
                kind: MessageKind::System,
                text: "Target does not exist".to_string(),
                hue: 120,
                font: 3,
                ..Default::default()
            }.into());
        }
    }
}

pub fn start_tile_info(
    mut exec: TextCommandQueue<TileInfo>,
    mut commands: Commands,
) {
    for (from, _) in exec.iter() {
        commands.spawn((
            WorldTargetRequest {
                client_entity: from,
                target_type: TargetType::Neutral,
            },
            ShowInfoCommand,
        ));
    }
}

pub fn tile_info(
    world: &World,
    type_registry: Res<AppTypeRegistry>,
    entity_positions: Res<EntityPositions>,
    clients: Query<(&NetClient, &ViewState)>,
    completed: Query<(Entity, &WorldTargetRequest, &WorldTargetResponse), With<ShowInfoCommand>>,
    mut commands: Commands,
) {
    for (entity, request, response) in completed.iter() {
        commands.entity(entity).despawn();

        let (client, view_state) = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };
        let map_id = view_state.map_id();
        let type_registry = type_registry.read();

        if let Some(position) = response.position {
            for (target, ..) in entity_positions.tree.iter_at_point(map_id, position.truncate()) {
                client.send_packet(UnicodeTextMessage {
                    kind: MessageKind::System,
                    text: format!("Entity {:?}", target),
                    hue: 120,
                    font: 3,
                    ..Default::default()
                }.into());

                let target_entity = world.entity(target);
                for component in target_entity.archetype().components() {
                    if let Some(info) = world.components().get_info(component) {
                        log::debug!("info");
                        if let Some(registration) = info.type_id().and_then(|id| type_registry.get(id)) {
                            log::debug!("reg");
                            if let Some(reflect_component) = registration.data::<ReflectComponent>() {
                                log::debug!("refl");
                                if let Some(reflected) = reflect_component.reflect(target_entity) {
                                    log::debug!("reflv");
                                    client.send_packet(UnicodeTextMessage {
                                        kind: MessageKind::System,
                                        text: format!("{:?}", reflected),
                                        hue: 120,
                                        font: 3,
                                        ..Default::default()
                                    }.into());
                                }
                            }
                        }

                        client.send_packet(UnicodeTextMessage {
                            kind: MessageKind::System,
                            text: format!("Has Component {}", info.name()),
                            hue: 120,
                            font: 3,
                            ..Default::default()
                        }.into());
                    }
                }
            }
        }
    }
}
