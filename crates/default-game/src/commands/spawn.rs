use std::sync::Arc;

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Commands, Query, Res, Resource};
use clap::Parser;
use glam::IVec2;

use yewoh::protocol::TargetType;
use yewoh_server::world::entity::{Container, Location, ParentContainer};
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse, WorldTargetRequest, WorldTargetResponse};
use yewoh_server::world::net::{NetClient, NetCommandsExt, ViewState};

use crate::commands::{TextCommand, TextCommandQueue};
use crate::data::prefab::{Prefab, PrefabCollection, PrefabCommandsExt};
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
    prefab_name: Arc<str>,
    prefab: Arc<Prefab>,
}

pub fn start_spawn(
    mut exec: TextCommandQueue<Spawn>,
    clients: Query<&NetClient>,
    prefabs: Res<PrefabCollection>,
    mut commands: Commands,
) {
    for (from, request) in exec.iter() {
        let client = match clients.get(from) {
            Ok(x) => x,
            _ => continue,
        };

        let (prefab_name, prefab) = match prefabs.get_key_value(&request.prefab) {
            Some((prefab_name, prefab)) => (prefab_name.clone(), prefab.clone()),
            None => {
                client.send_system_message_hue(format!("No such entity type '{}'", &request.prefab), hues::RED);
                continue;
            }
        };

        let spawn_request = SpawnRequest { prefab_name, prefab };

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

        // TODO: check whether location is obstructed.

        let (_, view_state) = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };
        let map_id = view_state.map_id();
        let prefab_instance = PrefabInstance { prefab_name: spawn.prefab_name.clone() };

        commands
            .spawn_empty()
            .insert_prefab(spawn.prefab.clone())
            .insert((
                prefab_instance,
                Persistent,
                Location {
                    map_id,
                    position,
                    direction: Default::default(),
                },
            ))
            .assign_network_id();
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

        let mut container = match containers.get_mut(target) {
            Ok(x) => x,
            _ => {
                client.send_system_message_hue(format!("Item is not a container"), hues::RED);
                continue;
            }
        };

        let prefab_instance = PrefabInstance { prefab_name: spawn.prefab_name.clone() };
        let new_entity = commands
            .spawn_empty()
            .insert_prefab(spawn.prefab.clone())
            .insert((
                prefab_instance,
                Persistent,
                ParentContainer {
                    parent: target,
                    position: IVec2::ZERO,
                    grid_index: 0,
                },
            ))
            .assign_network_id()
            .id();
        container.items.push(new_entity);
    }
}
