use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Query, Res, Resource};
use bevy::hierarchy::BuildChildren;
use clap::Parser;
use glam::IVec2;
use yewoh::protocol::TargetType;
use yewoh_server::world::entity::{Container, ContainerPosition, MapPosition};
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse, WorldTargetRequest, WorldTargetResponse};
use yewoh_server::world::net::{AssignNetId, NetClient, ViewState};

use crate::commands::{TextCommand, TextCommandQueue};
use crate::data::prefabs::{PrefabLibrary, PrefabLibraryEntityExt};
use crate::entities::{Persistent, PrefabInstance};
use crate::hues;
use crate::networking::NetClientExt;

#[derive(Parser, Resource)]
pub struct Spawn {
    #[arg(long, default_value = "false")]
    in_container: bool,

    prefab: String,
}

impl TextCommand for Spawn {
    fn aliases() -> &'static [&'static str] {
        &["spawn", "add"]
    }
}

#[derive(Debug, Clone, Component)]
pub struct SpawnRequest {
    prefab_name: String,
}

pub fn start_spawn(
    mut exec: TextCommandQueue<Spawn>,
    mut commands: Commands,
    prefabs: Res<PrefabLibrary>,
    clients: Query<(&NetClient, &ViewState)>,
) {
    for (from, request) in exec.iter() {
        if prefabs.get(&request.prefab).is_none() {
            let (client, ..) = match clients.get(from) {
                Ok(x) => x,
                _ => continue,
            };
            client.send_system_message_hue(format!("No such prefab '{}'", &request.prefab), hues::RED);
            continue;
        };

        let spawn_request = SpawnRequest { prefab_name: request.prefab };
        if request.in_container {
            commands
                .spawn((
                    EntityTargetRequest {
                        client_entity: from,
                        target_type: TargetType::Neutral,
                    },
                    spawn_request,
                ));
        } else {
            commands
                .spawn((
                    WorldTargetRequest {
                        client_entity: from,
                        target_type: TargetType::Neutral,
                    },
                    spawn_request,
                ));
        }
    }
}

pub fn spawn(
    completed_position: Query<(Entity, &SpawnRequest, &WorldTargetRequest, &WorldTargetResponse)>,
    completed_entity: Query<(Entity, &SpawnRequest, &EntityTargetRequest, &EntityTargetResponse)>,
    clients: Query<(&NetClient, &ViewState)>,
    mut containers: Query<&mut Container>,
    mut commands: Commands,
) {
    for (entity, spawn, request, response) in completed_position.iter() {
        commands.entity(entity).despawn();
        let position = match response.position {
            Some(x) => x,
            None => continue,
        };

        let (_, view_state, ..) = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        // TODO: check whether location is obstructed.

        let map_id = view_state.map_id();
        let prefab_instance = PrefabInstance { prefab_name: spawn.prefab_name.clone() };

        commands
            .spawn_empty()
            .fabricate_from_library(&spawn.prefab_name)
            .insert((
                prefab_instance,
                MapPosition {
                    map_id,
                    position,
                    direction: Default::default(),
                },
                Persistent,
                AssignNetId,
            ));
    }

    for (entity, spawn, request, response) in completed_entity.iter() {
        commands.entity(entity).despawn();

        let target = match response.target {
            Some(x) => x,
            None => continue,
        };

        let (client, ..) = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        match containers.get_mut(target) {
            Ok(_) => {},
            _ => {
                client.send_system_message_hue("Item is not a container".to_string(), hues::RED);
                continue;
            }
        };

        let prefab_instance = PrefabInstance { prefab_name: spawn.prefab_name.clone() };
        commands
            .spawn_empty()
            .fabricate_from_library(&spawn.prefab_name)
            .insert((
                prefab_instance,
                ContainerPosition {
                    position: IVec2::ZERO,
                    grid_index: 0,
                },
                Persistent,
                AssignNetId,
            ))
            .set_parent(target);
    }
}
