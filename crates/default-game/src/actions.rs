use bevy::prelude::*;
use smallvec::smallvec;
use yewoh::protocol;
use yewoh::protocol::{MoveConfirm, PickUpReject, MoveReject, ProfileResponse, SkillEntry, SkillLock, SkillsResponse, SkillsResponseKind, EntityFlags};
use yewoh_server::world::characters::{CharacterBodyType, NotorietyQuery, OnClientProfileRequest, OnClientSkillsRequest, WarMode};
use yewoh_server::world::combat::{AttackTarget, OnClientWarModeChanged};
use yewoh_server::world::connection::{NetClient, Possessing};
use yewoh_server::world::entity::{ContainedPosition, Direction, EquippedPosition, MapPosition, RootPosition};
use yewoh_server::world::input::{OnClientDrop, OnClientEquip, OnClientMove, OnClientPickUp};
use yewoh_server::world::items::{Container, ItemPosition, ItemQuantity, PositionQuery};
use yewoh_server::world::map::{Chunk, TileDataResource};
use yewoh_server::world::navigation::try_move_in_direction;
use yewoh_server::world::net_id::NetId;
use yewoh_server::world::spatial::SpatialQuery;
use yewoh_server::world::ServerSet;
use yewoh_server::world::sound::{OnClientSound, SoundKind};
use yewoh_server::world::view::ExpectedCharacterState;

use crate::data::prefabs::PrefabLibraryWorldExt;
use crate::entities::position::PositionExt;
use crate::entities::{Persistent, PrefabInstance};
use crate::entities::tooltips::MarkTooltipChanged;
use crate::items::common::{CanLift, DropSound, Stackable};
use crate::items::MAX_STACK;

#[derive(Debug, Clone, Component, Reflect)]
pub struct Held {
    pub held_entity: Entity,
    pub previous_position: Option<ItemPosition>,
}

#[derive(Debug, Clone, Component, Reflect)]
pub struct Holder {
    pub held_by: Entity,
}

pub fn on_client_move(
    spatial_query: SpatialQuery,
    chunk_query: Query<(&MapPosition, &Chunk)>,
    tile_data: Res<TileDataResource>,
    mut connection_query: Query<(&NetClient, &Possessing, &mut ExpectedCharacterState)>,
    mut characters: Query<(&mut MapPosition, &mut Direction, NotorietyQuery), Without<Chunk>>,
    mut events: EventReader<OnClientMove>,
) {
    for request in events.read() {
        let Ok((client, owned, mut expected)) = connection_query.get_mut(request.client_entity) else {
            continue;
        };

        let primary_entity = owned.entity;
        let Ok((mut map_position, mut direction, notoriety)) = characters.get_mut(primary_entity) else {
            continue;
        };

        if *direction != request.direction {
            *direction = request.direction;
        } else {
            match try_move_in_direction(&spatial_query, &chunk_query, &tile_data, *map_position, request.direction, Some(primary_entity)) {
                Ok(new_position) => {
                    *map_position = new_position;
                }
                Err(_) => {
                    client.send_packet(MoveReject {
                        sequence: request.sequence,
                        position: map_position.position,
                        direction: (*direction).into(),
                    });
                    continue;
                }
            }
        }

        let notoriety = notoriety.notoriety();
        client.send_packet(MoveConfirm {
            sequence: request.sequence,
            notoriety,
        });
        expected.position = *map_position;
    }
}

pub fn on_client_pick_up(
    clients: Query<(&NetClient, &Possessing)>,
    characters: Query<Option<&Held>>,
    targets: Query<
        (Entity, &PrefabInstance, &ItemQuantity, &RootPosition, PositionQuery),
        With<CanLift>,
    >,
    mut commands: Commands,
    mut events: EventReader<OnClientPickUp>,
    mut sounds: EventWriter<OnClientSound>,
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
            client.send_packet(PickUpReject::AlreadyHolding);
            continue;
        }

        let Ok((entity, prefab, quantity, root, position)) = targets.get(request.target) else {
            client.send_packet(PickUpReject::CannotLift);
            continue;
        };

        let item_position = position.item_position().unwrap();
        let quantity_left = (**quantity).saturating_sub(request.quantity.max(1));
        let quantity_taken = **quantity - quantity_left;

        sounds.send(OnClientSound {
            client_entity: request.client_entity,
            kind: SoundKind::OneShot,
            sound_id: 0x57,
            position: root.position,
        });

        let held_entity = if quantity_left == 0 {
            commands.entity(entity)
                .insert(Holder { held_by: character })
                .remove_position();
            entity
        } else {
            commands.entity(entity)
                .insert(ItemQuantity(quantity_left))
                .queue(MarkTooltipChanged);
            commands
                .fabricate_prefab(&prefab.prefab_name)
                .insert((
                    Persistent,
                    ItemQuantity(quantity_taken),
                ))
                .id()
        };

        commands.entity(character)
            .insert(Held {
                held_entity,
                previous_position: Some(item_position),
            });
    }
}

pub fn on_client_drop(
    clients: Query<&Possessing>,
    holders: Query<(&MapPosition, &Held)>,
    containers: Query<&Container>,
    stackable: Query<(&PrefabInstance, &ItemQuantity), With<Stackable>>,
    targets: Query<&DropSound>,
    mut commands: Commands,
    mut events: EventReader<OnClientDrop>,
    mut sounds: EventWriter<OnClientSound>,
) {
    for request in events.read() {
        let Ok(owner) = clients.get(request.client_entity) else {
            continue;
        };

        let character = owner.entity;
        let Ok((character_position, held)) = holders.get(character) else {
            continue;
        };

        let target = held.held_entity;
        if let Ok(sound) = targets.get(target) {
            sounds.send(OnClientSound {
                client_entity: request.client_entity,
                kind: SoundKind::OneShot,
                sound_id: **sound,
                position: character_position.position,
            });
        }

        if let Some(container_entity) = request.dropped_on {
            if containers.get(container_entity).is_ok() {
                commands.entity(target)
                    .remove::<Holder>()
                    .set_parent(container_entity)
                    .insert(ContainedPosition {
                        position: request.position.truncate(),
                        grid_index: request.grid_index,
                    });
            } else if let Ok([(a_prefab, a_quantity), (b_prefab, b_quantity)]) = stackable.get_many([target, container_entity]) {
                let new_quantity = (**a_quantity as u32) + (**b_quantity as u32);
                if a_prefab.prefab_name != b_prefab.prefab_name || new_quantity > MAX_STACK as u32 {
                    commands.entity(target)
                        .remove::<Holder>()
                        .insert(MapPosition {
                            position: character_position.position,
                            map_id: character_position.map_id,
                            ..Default::default()
                        });
                } else {
                    commands.entity(target).despawn_recursive();
                    commands.entity(container_entity)
                        .insert(ItemQuantity(new_quantity as u16))
                        .queue(MarkTooltipChanged);
                }
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
            client.send_packet(PickUpReject::BelongsToAnother);
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

        client.send_packet(ProfileResponse {
            target_id: target_id.id,
            header: "Supreme Commander".to_string(),
            footer: "Static Profile".to_string(),
            profile: "Bio".to_string(),
        });
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

        client.send_packet(SkillsResponse {
            kind: SkillsResponseKind::FullWithCaps,
            skills: smallvec![
                SkillEntry {
                    id: 1,
                    value: 724,
                    raw_value: 701,
                    lock: SkillLock::Up,
                    cap: 1200,
                }
            ],
        });
    }
}

pub fn on_client_war_mode_changed(
    mut commands: Commands,
    mut clients: Query<(&NetClient, &Possessing, &mut ExpectedCharacterState)>,
    mut characters: Query<&mut WarMode>,
    mut events: EventReader<OnClientWarModeChanged>,
) {
    for request in events.read() {
        let Ok((client, owned, mut expected)) = clients.get_mut(request.client_entity) else {
            continue;
        };

        let Ok(mut war_mode) = characters.get_mut(owned.entity) else {
            continue;
        };

        if request.war_mode {
            expected.flags |= EntityFlags::WAR_MODE.bits();
            **war_mode = true;
        } else {
            expected.flags &= !EntityFlags::WAR_MODE.bits();
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
                on_client_pick_up,
                on_client_drop,
                on_client_equip,
                on_client_move,
                on_client_profile_request,
                on_client_skills_request,
            ).in_set(ServerSet::HandlePackets),
        ));
}
