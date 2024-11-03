use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Query, Res, Resource};
use bevy::hierarchy::BuildChildren;
use clap::Parser;
use yewoh::protocol::TargetType;
use yewoh_server::world::entity::{Container, ContainedPosition, MapPosition};
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse, WorldTargetRequest, WorldTargetResponse};
use yewoh_server::world::connection::{NetClient};
use yewoh_server::world::view::ViewKey;
use crate::commands::{TextCommand, TextCommandQueue};
use crate::data::prefabs::{PrefabLibrary, PrefabLibraryEntityExt};
use crate::entities::Persistent;
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
    clients: Query<&NetClient>,
) {
    for (from, request) in exec.iter() {
        if prefabs.get(&request.prefab).is_none() {
            let client = match clients.get(from) {
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
    clients: Query<(&NetClient, &ViewKey)>,
    mut containers: Query<&mut Container>,
    mut commands: Commands,
) {
    for (entity, spawn, request, response) in completed_position.iter() {
        commands.entity(entity).despawn();
        let position = match response.position {
            Some(x) => x,
            None => continue,
        };

        let (_, view_key, ..) = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        // TODO: check whether location is obstructed.

        let map_id = view_key.map_id;
        commands
            .spawn_empty()
            .fabricate_prefab(&spawn.prefab_name)
            .insert((
                Persistent,
                MapPosition {
                    map_id,
                    position,
                    direction: Default::default(),
                },
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

        commands
            .spawn_empty()
            .fabricate_prefab(&spawn.prefab_name)
            .insert((
                Persistent,
                ContainedPosition::default(),
            ))
            .set_parent(target);
    }
}
