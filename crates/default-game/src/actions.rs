use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;

use yewoh::protocol::{Swing, CharacterProfile, ContextMenuEntry, DamageDealt, EntityFlags, MoveConfirm, MoveEntityReject, OpenContainer, OpenPaperDoll, ProfileResponse, SkillEntry, SkillLock, Skills, SkillsResponse, SkillsResponseKind, WarMode, CharacterAnimation};
use yewoh_server::world::entity::{Character, Container, EquippedBy, Flags, Graphic, MapPosition, Notorious, ParentContainer, Quantity};
use yewoh_server::world::events::{ContextMenuEvent, DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, PickUpEvent, ProfileEvent, ReceivedPacketEvent, RequestSkillsEvent, SingleClickEvent};
use yewoh_server::world::input::ContextMenuRequest;
use yewoh_server::world::map::Chunk;
use yewoh_server::world::net::{make_container_contents_packet, NetClient, NetEntity, NetEntityLookup, NetOwned, PlayerState};
use yewoh_server::world::spatial::EntitySurfaces;

#[derive(Debug, Clone, Component, Reflect)]
pub struct Held {
    pub held_entity: Entity,
}

#[derive(Debug, Clone, Component, Reflect)]
pub struct Holder {
    pub held_by: Entity,
}

pub fn handle_move(
    mut events: EventReader<MoveEvent>,
    surfaces: Res<EntitySurfaces>,
    map_chunks: Query<&Chunk>,
    connection_query: Query<(&NetClient, &NetOwned)>,
    mut character_query: Query<(&mut MapPosition, &mut PlayerState, &Notorious)>,
) {
    for MoveEvent { client_entity: connection, request } in events.iter() {
        let connection = *connection;
        let (client, owned) = match connection_query.get(connection) {
            Ok(x) => x,
            _ => continue,
        };

        let primary_entity = owned.primary_entity;
        let (mut map_position, mut state, notoriety) = match character_query.get_mut(primary_entity) {
            Ok(x) => x,
            _ => continue,
        };

        if map_position.direction != request.direction {
            map_position.direction = request.direction;
        } else {
            // Step forward and up 10 units, then drop the character down onto their destination.
            let mut test_position = map_position.position + request.direction.as_vec2().extend(10);
            let mut new_z = 0;

            for (entity, min, _) in surfaces.tree.iter_at_column(map_position.map_id, test_position.truncate()) {
                if let Ok(chunk) = map_chunks.get(entity) {
                    let chunk_pos = test_position - min;
                    let (_, z) = chunk.map_chunk.get(chunk_pos.x as usize, chunk_pos.y as usize);
                    let z = z as i32;
                    if z <= test_position.z {
                        new_z = new_z.max(z);
                    }
                } else {
                    let z = min.z;
                    if z <= test_position.z {
                        new_z = new_z.max(z);
                    }
                }
            }

            test_position.z = new_z;
            map_position.position = test_position;
        }

        state.position = *map_position;

        let notoriety = **notoriety;
        client.send_packet(MoveConfirm {
            sequence: request.sequence,
            notoriety,
        }.into());
    }
}

pub fn handle_single_click(
    mut click_events: EventReader<SingleClickEvent>,
    mut commands: Commands,
) {
    for SingleClickEvent { client_entity: client, target } in click_events.iter() {
        let client_entity = *client;
        let target = match target {
            Some(x) => *x,
            None => continue,
        };

        commands.spawn()
            .insert(ContextMenuRequest {
                client_entity,
                target,
                entries: Vec::new(),
            });
    }
}

pub fn handle_double_click(
    mut events: EventReader<DoubleClickEvent>,
    clients: Query<&NetClient>,
    target_query: Query<(&NetEntity, Option<&Character>, Option<&Container>)>,
    content_query: Query<(&NetEntity, &ParentContainer, &Graphic, Option<&Quantity>)>,
) {
    for DoubleClickEvent { client_entity: client, target } in events.iter() {
        let client = match clients.get(*client) {
            Ok(x) => x,
            _ => continue,
        };
        let target = match target {
            Some(x) => *x,
            None => continue,
        };

        let (net, character, container) = match target_query.get(target) {
            Ok(e) => e,
            _ => continue,
        };

        if character.is_some() {
            client.send_packet(OpenPaperDoll {
                id: net.id,
                text: "Me, Myself and I".into(),
                flags: Default::default(),
            }.into());
        }

        if let Some(container) = container {
            client.send_packet(OpenContainer {
                id: net.id,
                gump_id: container.gump_id,
            }.into());
            client.send_packet(make_container_contents_packet(net.id, container, &content_query).into());
        }
    }
}

pub fn handle_pick_up(
    mut events: EventReader<PickUpEvent>,
    clients: Query<(&NetClient, &NetOwned)>,
    characters: Query<Option<&Held>>,
    targets: Query<(Entity, Option<&MapPosition>, Option<&ParentContainer>, Option<&EquippedBy>)>,
    mut containers: Query<&mut Container>,
    mut character_equipment: Query<&mut Character>,
    mut commands: Commands,
) {
    for event in events.iter() {
        let (client, owner) = match clients.get(event.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let character = owner.primary_entity;
        let held = match characters.get(character) {
            Ok(x) => x,
            _ => continue,
        };

        if held.is_some() {
            client.send_packet(MoveEntityReject::AlreadyHolding.into());
            continue;
        }

        let (entity, position, container, equipped) = match targets.get(event.target) {
            Ok(x) => x,
            Err(_) => {
                client.send_packet(MoveEntityReject::CannotLift.into());
                continue;
            }
        };

        if let Some(_) = position {
            commands.entity(entity)
                .insert(Holder { held_by: character })
                .remove::<MapPosition>();
        } else if let Some(container) = container {
            let mut container = containers.get_mut(container.parent).unwrap();
            container.items.retain(|v| v != &entity);
            commands.entity(entity)
                .insert(Holder { held_by: character })
                .remove::<ParentContainer>();
        } else if let Some(equipped) = equipped {
            let mut equipped_character = character_equipment.get_mut(equipped.parent).unwrap();
            equipped_character.equipment.retain(|e| e != &entity);
            commands.entity(entity)
                .insert(Holder { held_by: character })
                .remove::<EquippedBy>();
        } else {
            // Not sure where this item is, do nothing.
            client.send_packet(MoveEntityReject::OutOfRange.into());
            continue;
        }

        commands.entity(character)
            .insert(Held { held_entity: entity });
    }
}

pub fn handle_drop(
    mut events: EventReader<DropEvent>,
    clients: Query<(&NetClient, &NetOwned)>,
    characters: Query<(&MapPosition, &Held)>,
    mut containers: Query<&mut Container>,
    mut commands: Commands,
) {
    for event in events.iter() {
        let (client, owner) = match clients.get(event.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let character = owner.primary_entity;
        let (character_position, held) = match characters.get(character) {
            Ok(x) => x,
            _ => continue,
        };

        if held.held_entity != event.target {
            client.send_packet(MoveEntityReject::BelongsToAnother.into());
            continue;
        }

        let target = event.target;

        if let Some(container_entity) = event.dropped_on {
            if let Ok(mut container) = containers.get_mut(container_entity) {
                container.items.push(target);
                commands.entity(target)
                    .remove::<Holder>()
                    .insert(ParentContainer {
                        parent: event.dropped_on.unwrap(),
                        position: event.position.truncate(),
                        grid_index: event.grid_index,
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
                    position: event.position,
                    map_id: character_position.map_id,
                    ..Default::default()
                });
        }

        commands.entity(character)
            .remove::<Held>();
    }
}

pub fn handle_equip(
    mut events: EventReader<EquipEvent>,
    clients: Query<(&NetClient, &NetOwned)>,
    characters: Query<(&MapPosition, &Held)>,
    mut loadouts: Query<&mut Character>,
    mut commands: Commands,
) {
    for event in events.iter() {
        let (client, owner) = match clients.get(event.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let character = owner.primary_entity;
        let (character_position, held) = match characters.get(character) {
            Ok(x) => x,
            _ => continue,
        };

        if held.held_entity != event.target {
            client.send_packet(MoveEntityReject::BelongsToAnother.into());
            continue;
        }

        let target = event.target;
        if let Ok(mut target_character) = loadouts.get_mut(event.character) {
            target_character.equipment.push(target);
            commands.entity(target)
                .remove::<Holder>()
                .insert(EquippedBy {
                    parent: event.character,
                    slot: event.slot,
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

pub fn handle_context_menu(
    lookup: Res<NetEntityLookup>,
    clients: Query<(&NetClient, &NetOwned)>,
    characters: Query<&NetEntity>,
    mut context_requests: Query<&mut ContextMenuRequest>,
    mut context_events: EventReader<ContextMenuEvent>,
) {
    for mut request in context_requests.iter_mut() {
        request.entries.push(ContextMenuEntry {
            id: 0,
            text_id: 3000489,
            hue: None,
            flags: Default::default()
        });
    }

    for ContextMenuEvent { client_entity, target, .. } in context_events.iter() {
        let (client, owned) = match clients.get(*client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let net = match characters.get(owned.primary_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let target_id = match lookup.ecs_to_net(*target) {
            Some(x) => x,
            _ => continue,
        };

        client.send_packet(Swing {
            attacker_id: net.id,
            target_id,
        }.into());
        client.send_packet(CharacterAnimation {
            target_id: net.id,
            animation_id: 9,
            frame_count: 7,
            repeat_count: 1,
            reverse: false,
            speed: 0,
        }.into());
        client.send_packet(CharacterAnimation {
            target_id,
            animation_id: 7,
            frame_count: 5,
            repeat_count: 1,
            reverse: false,
            speed: 0,
        }.into());
        client.send_packet(DamageDealt {
            target_id,
            damage: 1337,
        }.into());
    }
}

pub fn handle_profile_requests(
    lookup: Res<NetEntityLookup>,
    clients: Query<&NetClient>,
    mut requests: EventReader<ProfileEvent>,
) {
    for request in requests.iter() {
        let client_entity = request.client_entity;
        let client = match clients.get(client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let target_id = match lookup.ecs_to_net(request.target) {
            Some(x) => x,
            _ => continue,
        };

        client.send_packet(CharacterProfile::Response(ProfileResponse {
            target_id,
            header: "Supreme Commander".to_string(),
            footer: "Static Profile".to_string(),
            profile: "Bio".to_string()
        }).into());
    }
}

pub fn handle_skills_requests(
    clients: Query<&NetClient>,
    mut requests: EventReader<RequestSkillsEvent>,
) {
    for request in requests.iter() {
        let client_entity = request.client_entity;
        let client = match clients.get(client_entity) {
            Ok(x) => x,
            _ => continue,
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
            ]
        }).into());
    }
}

pub fn handle_war_mode(
    clients: Query<(&NetClient, &NetOwned)>,
    mut characters: Query<&mut Flags>,
    mut new_packets: EventReader<ReceivedPacketEvent>,
) {
    for ReceivedPacketEvent { client_entity, packet } in new_packets.iter() {
        let packet = match packet.downcast::<WarMode>() {
            Some(x) => x,
            _ => continue,
        };

        let (client, owned) = match clients.get(*client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let mut flags = match characters.get_mut(owned.primary_entity) {
            Ok(x) => x,
            _ => continue,
        };

        if packet.war {
            flags.flags |= EntityFlags::WAR_MODE;
        } else {
            flags.flags &= !EntityFlags::WAR_MODE;
        }

        client.send_packet(packet.clone().into());
    }
}