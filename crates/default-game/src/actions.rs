use bevy::prelude::*;
use yewoh::protocol;
use yewoh::protocol::{CharacterAnimation, CharacterProfile, ContextMenu, ContextMenuEntry, DamageDealt, ExtendedCommand, MoveConfirm, MoveEntityReject, MoveReject, OpenPaperDoll, ProfileResponse, SkillEntry, SkillLock, Skills, SkillsResponse, SkillsResponseKind, Swing};
use yewoh::types::FixedString;
use yewoh_server::world::characters::{CharacterBodyType, CharacterNotoriety, OnClientProfileRequest, OnClientSkillsRequest, WarMode};
use yewoh_server::world::combat::{AttackTarget, OnClientWarModeChanged};
use yewoh_server::world::connection::{NetClient, Possessing};
use yewoh_server::world::entity::{ContainedPosition, EquippedPosition, MapPosition};
use yewoh_server::world::input::{OnClientContextMenuAction, OnClientContextMenuRequest, OnClientDoubleClick, OnClientDrop, OnClientEquip, OnClientMove, OnClientPickUp, OnClientSingleClick};
use yewoh_server::world::items::{Container, OnContainerOpen};
use yewoh_server::world::map::{Chunk, TileDataResource};
use yewoh_server::world::navigation::try_move_in_direction;
use yewoh_server::world::net_id::NetId;
use yewoh_server::world::ServerSet;
use yewoh_server::world::spatial::SpatialQuery;
use yewoh_server::world::view::Synchronized;

#[derive(Debug, Clone, Component, Reflect)]
pub struct Held {
    pub held_entity: Entity,
}

#[derive(Debug, Clone, Component, Reflect)]
pub struct Holder {
    pub held_by: Entity,
}

pub fn on_client_move(
    spatial_query: SpatialQuery,
    chunk_query: Query<(&MapPosition, &Chunk)>,
    tile_data: Res<TileDataResource>,
    connection_query: Query<(&NetClient, &Possessing)>,
    mut characters: Query<(&mut MapPosition, &CharacterNotoriety), Without<Chunk>>,
    mut events: EventReader<OnClientMove>,
) {
    for request in events.read() {
        let Ok((client, owned)) = connection_query.get(request.client_entity) else {
            continue;
        };

        let primary_entity = owned.entity;
        let Ok((mut map_position, notoriety)) = characters.get_mut(primary_entity) else {
            continue;
        };

        if map_position.direction != request.request.direction {
            map_position.direction = request.request.direction;
        } else {
            match try_move_in_direction(&spatial_query, &chunk_query, &tile_data, *map_position, request.request.direction, Some(primary_entity)) {
                Ok(new_position) => {
                    *map_position = new_position;
                }
                Err(_) => {
                    client.send_packet(MoveReject {
                        sequence: request.request.sequence,
                        position: map_position.position,
                        direction: map_position.direction,
                    });
                }
            }
        }

        let notoriety = **notoriety;
        client.send_packet(MoveConfirm {
            sequence: request.request.sequence,
            notoriety,
        });
    }
}

pub fn on_client_single_click(
    mut commands: Commands,
    mut events: EventReader<OnClientSingleClick>,
) {
    for request in events.read() {
        let client_entity = request.client_entity;
        commands.trigger_targets(OnClientContextMenuRequest {
            client_entity,
            target: request.target,
        }, client_entity);
    }
}

pub fn on_client_double_click(
    mut opened_containers: EventWriter<OnContainerOpen>,
    clients: Query<&NetClient, With<Synchronized>>,
    target_query: Query<(&NetId, Option<&CharacterBodyType>, Option<&Container>)>,
    mut events: EventReader<OnClientDoubleClick>,
) {
    for request in events.read() {
        let client_entity = request.client_entity;
        let Ok(client) = clients.get(client_entity) else {
            continue;
        };

        let Ok((net, character, container)) = target_query.get(request.target) else {
            continue;
        };

        if character.is_some() {
            client.send_packet(OpenPaperDoll {
                id: net.id,
                text: FixedString::from_str("Me, Myself and I"),
                flags: Default::default(),
            });
        }

        if container.is_some() {
            opened_containers.send(OnContainerOpen {
                client_entity,
                container: request.target,
            });
        }
    }
}

pub fn on_client_pick_up(
    clients: Query<(&NetClient, &Possessing)>,
    characters: Query<Option<&Held>>,
    targets: Query<(Entity, Option<&MapPosition>, Option<&ContainedPosition>, Option<&EquippedPosition>)>,
    mut commands: Commands,
    mut events: EventReader<OnClientPickUp>,
) {
    for request in events.read() {
        let Ok((client, owner)) = clients.get(request.client_entity) else {
            continue;
        };

        let character = owner.entity;
        let Ok(held) = characters.get(character) else {
            continue;
        };

        if held.is_some() {
            client.send_packet(MoveEntityReject::AlreadyHolding);
            continue;
        }

        let Ok((entity, position, container, equipped)) = targets.get(request.target) else {
            client.send_packet(MoveEntityReject::CannotLift);
            continue;
        };

        if position.is_some() {
            commands.entity(entity)
                .insert(Holder { held_by: character })
                .remove::<MapPosition>();
        } else if container.is_some() {
            commands.entity(entity)
                .insert(Holder { held_by: character })
                .remove_parent()
                .remove::<ContainedPosition>();
        } else if equipped.is_some() {
            commands.entity(entity)
                .insert(Holder { held_by: character })
                .remove_parent()
                .remove::<EquippedPosition>();
        } else {
            // Not sure where this item is, do nothing.
            client.send_packet(MoveEntityReject::OutOfRange);
            continue;
        }

        commands.entity(character)
            .insert(Held { held_entity: entity });
    }
}

pub fn on_client_drop(
    clients: Query<(&NetClient, &Possessing)>,
    characters: Query<(&MapPosition, &Held)>,
    mut containers: Query<&mut Container>,
    mut commands: Commands,
    mut events: EventReader<OnClientDrop>,
) {
    for request in events.read() {
        let Ok((client, owner)) = clients.get(request.client_entity) else {
            continue;
        };

        let character = owner.entity;
        let Ok((character_position, held)) = characters.get(character) else {
            continue;
        };

        if held.held_entity != request.target {
            client.send_packet(MoveEntityReject::BelongsToAnother);
            continue;
        }

        let target = request.target;

        if let Some(container_entity) = request.dropped_on {
            if containers.get_mut(container_entity).is_ok() {
                commands.entity(target)
                    .remove::<Holder>()
                    .set_parent(request.dropped_on.unwrap())
                    .insert(ContainedPosition {
                        position: request.position.truncate(),
                        grid_index: request.grid_index,
                    });
            } else {
                commands.entity(target)
                    .remove::<Holder>()
                    .insert(MapPosition {
                        position: character_position.position,
                        map_id: character_position.map_id,
                        ..Default::default()
                    });
            }
        } else {
            commands.entity(target)
                .remove::<Holder>()
                .insert(MapPosition {
                    position: request.position,
                    map_id: character_position.map_id,
                    ..Default::default()
                });
        }

        commands.entity(character)
            .remove::<Held>();
    }
}

pub fn on_client_equip(
    clients: Query<(&NetClient, &Possessing)>,
    characters: Query<(&MapPosition, &Held)>,
    mut loadouts: Query<&mut CharacterBodyType>,
    mut commands: Commands,
    mut events: EventReader<OnClientEquip>,
) {
    for request in events.read() {
        let Ok((client, owner)) = clients.get(request.client_entity) else {
            continue;
        };

        let character = owner.entity;
        let Ok((character_position, held)) = characters.get(character) else {
            continue;
        };

        if held.held_entity != request.target {
            client.send_packet(MoveEntityReject::BelongsToAnother);
            continue;
        }

        let target = request.target;
        if loadouts.get_mut(request.character).is_ok() {
            commands.entity(target)
                .remove::<Holder>()
                .set_parent(request.character)
                .insert(EquippedPosition {
                    slot: request.slot,
                });
        } else {
            commands.entity(target)
                .remove::<Holder>()
                .insert(MapPosition {
                    position: character_position.position,
                    map_id: character_position.map_id,
                    ..Default::default()
                });
        }

        commands.entity(character)
            .remove::<Held>();
    }
}

pub fn on_client_context_menu_request(
    net_ids: Query<&NetId>,
    clients: Query<(&NetClient, &Possessing)>,
    mut events: EventReader<OnClientContextMenuRequest>,
) {
    for request in events.read() {
        let Ok((client, _)) = clients.get(request.client_entity) else {
            continue;
        };

        let Ok(net_id) = net_ids.get(request.target) else {
            continue;
        };

        client.send_packet(ExtendedCommand::ContextMenu(ContextMenu {
            target_id: net_id.id,
            entries: vec![
                ContextMenuEntry {
                    id: 0,
                    text_id: 3000489,
                    hue: None,
                    flags: Default::default(),
                },
            ],
        }));
    }
}

pub fn on_client_context_menu_action(
    net_ids: Query<&NetId>,
    clients: Query<(&NetClient, &Possessing)>,
    mut events: EventReader<OnClientContextMenuAction>,
) {
    for request in events.read() {
        let Ok((client, owned)) = clients.get(request.client_entity) else {
            continue;
        };

        let Ok(net) = net_ids.get(owned.entity) else {
            continue;
        };

        let Ok(target_id) = net_ids.get(request.target) else {
            continue;
        };

        client.send_packet(Swing {
            attacker_id: net.id,
            target_id: target_id.id,
        });
        client.send_packet(CharacterAnimation {
            target_id: net.id,
            animation_id: 9,
            frame_count: 7,
            repeat_count: 1,
            reverse: false,
            speed: 0,
        });
        client.send_packet(CharacterAnimation {
            target_id: target_id.id,
            animation_id: 7,
            frame_count: 5,
            repeat_count: 1,
            reverse: false,
            speed: 0,
        });
        client.send_packet(DamageDealt {
            target_id: target_id.id,
            damage: 1337,
        });
    }
}

pub fn on_client_profile_request(
    net_ids: Query<&NetId>,
    clients: Query<&NetClient>,
    mut events: EventReader<OnClientProfileRequest>,
) {
    for request in events.read() {
        let Ok(client) = clients.get(request.client_entity) else {
            continue;
        };

        let Ok(target_id) = net_ids.get(request.target) else {
            continue;
        };

        client.send_packet(CharacterProfile::Response(ProfileResponse {
            target_id: target_id.id,
            header: "Supreme Commander".to_string(),
            footer: "Static Profile".to_string(),
            profile: "Bio".to_string(),
        }));
    }
}

pub fn on_client_skills_request(
    clients: Query<&NetClient>,
    mut events: EventReader<OnClientSkillsRequest>,
) {
    for request in events.read() {
        let Ok(client) = clients.get(request.client_entity) else {
            continue;
        };

        client.send_packet(Skills::Response(SkillsResponse {
            kind: SkillsResponseKind::FullWithCaps,
            skills: vec![
                SkillEntry {
                    id: 1,
                    value: 724,
                    raw_value: 701,
                    lock: SkillLock::Up,
                    cap: 1200,
                }
            ],
        }));
    }
}

pub fn on_client_war_mode_changed(
    mut commands: Commands,
    clients: Query<(&NetClient, &Possessing)>,
    mut characters: Query<&mut WarMode>,
    mut events: EventReader<OnClientWarModeChanged>,
) {
    for request in events.read() {
        let Ok((client, owned)) = clients.get(request.client_entity) else {
            continue;
        };

        let Ok(mut war_mode) = characters.get_mut(owned.entity) else {
            continue;
        };

        if request.war_mode {
            **war_mode = true;
        } else {
            **war_mode = false;
            commands.entity(owned.entity).remove::<AttackTarget>();
        }

        client.send_packet(protocol::WarMode { war: **war_mode });
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_systems(First, (
            (
                on_client_war_mode_changed,
                on_client_single_click,
                on_client_double_click,
                on_client_pick_up,
                on_client_drop,
                on_client_equip,
                on_client_move,
                on_client_context_menu_request,
                on_client_profile_request,
                on_client_skills_request,
            ).in_set(ServerSet::HandlePackets),
        ));
}
