use bevy::asset::{Assets, Handle};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Query, Res, Resource};
use bevy::prelude::AssetServer;
use clap::Parser;
use glam::IVec2;
use bevy_fabricator::{Fabricate, FabricateExt, Fabricator};
use yewoh::protocol::TargetType;
use yewoh_server::world::entity::{Container, Location, ParentContainer};
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse, WorldTargetRequest, WorldTargetResponse};
use yewoh_server::world::net::{NetClient, NetCommandsExt, ViewState};

use crate::commands::{TextCommand, TextCommandQueue};
use crate::entities::PrefabInstance;
use crate::hues;
use crate::networking::NetClientExt;
use crate::persistence::PersistenceCommandsExt;

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
    fabricator: Handle<Fabricator>,
}

pub fn start_spawn(
    mut exec: TextCommandQueue<Spawn>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for (from, request) in exec.iter() {
        let fabricator = asset_server.load(&request.prefab);
        let spawn_request = SpawnRequest { fabricator };

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
    fabricators: Res<Assets<Fabricator>>,
    asset_server: Res<AssetServer>,
    mut containers: Query<&mut Container>,
    mut commands: Commands,
) {
    for (entity, spawn, request, response) in completed_position.iter() {
        let (client, view_state, ..) = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let maybe_request = Fabricate::with_handle(spawn.fabricator.clone())
            .to_request(&fabricators, Some(&asset_server));
        let request = match maybe_request {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(err) => {
                commands.entity(entity).despawn();
                let path = asset_server.get_path(&spawn.fabricator).unwrap();
                client.send_system_message_hue(format!("Failed to load prefab '{path:?}': {err}"), hues::RED);
                continue;
            }
        };

        commands.entity(entity).despawn();
        let position = match response.position {
            Some(x) => x,
            None => continue,
        };

        // TODO: check whether location is obstructed.

        let map_id = view_state.map_id();
        let prefab_instance = PrefabInstance { fabricator: spawn.fabricator.clone() };

        commands
            .spawn_empty()
            .fabricate(request)
            .insert((
                prefab_instance,
                Location {
                    map_id,
                    position,
                    direction: Default::default(),
                },
            ))
            .make_persistent()
            .assign_network_id();
    }

    for (entity, spawn, request, response) in completed_entity.iter() {
        let (client, ..) = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let maybe_request = Fabricate::with_handle(spawn.fabricator.clone())
            .to_request(&fabricators, Some(&asset_server));
        let request = match maybe_request {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(err) => {
                commands.entity(entity).despawn();
                let path = asset_server.get_path(&spawn.fabricator).unwrap();
                client.send_system_message_hue(format!("Failed to load prefab '{path:?}': {err}"), hues::RED);
                continue;
            }
        };

        commands.entity(entity).despawn();

        let target = match response.target {
            Some(x) => x,
            None => continue,
        };

        let mut container = match containers.get_mut(target) {
            Ok(x) => x,
            _ => {
                client.send_system_message_hue("Item is not a container".to_string(), hues::RED);
                continue;
            }
        };

        let prefab_instance = PrefabInstance { fabricator: spawn.fabricator.clone() };
        let new_entity = commands
            .spawn_empty()
            .fabricate(request)
            .insert((
                prefab_instance,
                ParentContainer {
                    parent: target,
                    position: IVec2::ZERO,
                    grid_index: 0,
                },
            ))
            .make_persistent()
            .assign_network_id()
            .id();
        container.items.push(new_entity);
    }
}
