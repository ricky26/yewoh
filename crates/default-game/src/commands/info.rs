use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use bevy_reflect::TypeRegistry;
use clap::Parser;

use yewoh::protocol::TargetType;
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse, WorldTargetRequest, WorldTargetResponse};
use yewoh_server::world::net::{NetClient, ViewState};
use yewoh_server::world::spatial::EntityPositions;

use crate::commands::{TextCommand, TextCommandQueue};
use crate::networking::NetClientExt;

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

pub fn start_info(
    mut exec: TextCommandQueue<Info>,
    mut exec_tile: TextCommandQueue<TileInfo>,
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

fn send_entity_info(world: &World, type_registry: &TypeRegistry, client: &NetClient, entity: Entity) {
    let add_line = |text| client.send_system_message(text);
    add_line(format!("Entity {:?}", entity));

    let target_entity = world.entity(entity);
    for component in target_entity.archetype().components() {
        if let Some(info) = world.components().get_info(component) {
            let reflected = info.type_id()
                .and_then(|id| type_registry.get(id))
                .and_then(|r| r.data::<ReflectComponent>())
                .and_then(|r| r.reflect(target_entity));

            if let Some(reflected) = reflected {
                add_line(format!("{:?}", reflected));
            } else {
                add_line(format!("{}", info.name()));
            }
        } else {
            add_line(format!("C={:?}", component));
        }
    }
}

pub fn info(
    world: &World,
    type_registry: Res<AppTypeRegistry>,
    entity_positions: Res<EntityPositions>,
    clients: Query<(&NetClient, &ViewState)>,
    completed_tile: Query<(Entity, &WorldTargetRequest, &WorldTargetResponse), With<ShowInfoCommand>>,
    completed_entity: Query<(Entity, &EntityTargetRequest, &EntityTargetResponse), With<ShowInfoCommand>>,
    mut commands: Commands,
) {
    let type_registry = type_registry.read();

    for (entity, request, response) in completed_tile.iter() {
        commands.entity(entity).despawn();

        let (client, view_state) = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };
        let map_id = view_state.map_id();

        if let Some(position) = response.position {
            for (target, ..) in entity_positions.tree.iter_at_point(map_id, position.truncate()) {
                send_entity_info(world, &type_registry, client, target);
            }
        }
    }

    for (entity, request, response) in completed_entity.iter() {
        commands.entity(entity).despawn();
        let (client, ..) = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        if let Some(target) = response.target {
            send_entity_info(world, &type_registry, client, target);
        }
    }
}
