use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;

use yewoh::protocol::{CharacterAnimation, CharacterProfile, ContextMenuEntry, DamageDealt, EntityFlags, MoveConfirm, MoveEntityReject, MoveReject, OpenPaperDoll, ProfileResponse, SkillEntry, SkillLock, Skills, SkillsResponse, SkillsResponseKind, Swing, WarMode};
use yewoh::types::FixedString;
use yewoh_server::world::entity::{AttackTarget, Character, CharacterEquipped, Container, EquippedBy, Flags, Location, Notorious, ParentContainer};
use yewoh_server::world::events::{ContextMenuEvent, DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, PickUpEvent, ProfileEvent, ReceivedPacketEvent, RequestSkillsEvent, SingleClickEvent};
use yewoh_server::world::input::ContextMenuRequest;
use yewoh_server::world::map::TileDataResource;
use yewoh_server::world::navigation::try_move_in_direction;
use yewoh_server::world::net::{ContainerOpenedEvent, NetClient, NetEntity, NetEntityLookup, Possessing};
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
    tile_data: Res<TileDataResource>,
    connection_query: Query<(&NetClient, &Possessing)>,
    mut characters: Query<(&mut Location, &Notorious)>,
) {
    for MoveEvent { client_entity: connection, request } in events.read() {
        let connection = *connection;
        let (client, owned) = match connection_query.get(connection) {
            Ok(x) => x,
            _ => continue,
        };

        let primary_entity = owned.entity;
        let (mut map_position, notoriety) = match characters.get_mut(primary_entity) {
            Ok(x) => x,
            _ => continue,
        };

        if map_position.direction != request.direction {
            map_position.direction = request.direction;
        } else {
            match try_move_in_direction(&surfaces, &tile_data, *map_position, request.direction, Some(primary_entity)) {
                Ok(new_position) => {
                    *map_position = new_position;
                }
                Err(_) => {
                    client.send_packet(MoveReject {
                        sequence: request.sequence,
                        position: map_position.position,
                        direction: map_position.direction,
                    }.into());
                }
            }
        }

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
    for SingleClickEvent { client_entity: client, target } in click_events.read() {
        let client_entity = *client;
        let target = match target {
            Some(x) => *x,
            None => continue,
        };

        commands.spawn(ContextMenuRequest {
            client_entity,
            target,
            entries: Vec::new(),
        });
    }
}

pub fn handle_double_click(
    mut events: EventReader<DoubleClickEvent>,
    mut clients: Query<&NetClient>,
    mut opened_containers: EventWriter<ContainerOpenedEvent>,
    target_query: Query<(&NetEntity, Option<&Character>, Option<&Container>)>,
) {
    for DoubleClickEvent { client_entity, target } in events.read() {
        let client = match clients.get_mut(*client_entity) {
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
                text: FixedString::from_str("Me, Myself and I"),
                flags: Default::default(),
            }.into());
        }

        if container.is_some() {
            opened_containers.send(ContainerOpenedEvent {
                client_entity: *client_entity,
                container: target,
            });
        }
    }
}

pub fn handle_pick_up(
    mut events: EventReader<PickUpEvent>,
    clients: Query<(&NetClient, &Possessing)>,
    characters: Query<Option<&Held>>,
    targets: Query<(Entity, Option<&Location>, Option<&ParentContainer>, Option<&EquippedBy>)>,
    mut containers: Query<&mut Container>,
    mut character_equipment: Query<&mut Character>,
    mut commands: Commands,
) {
    for event in events.read() {
        let (client, owner) = match clients.get(event.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let character = owner.entity;
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
                .remove::<Location>();
        } else if let Some(container) = container {
            let mut container = containers.get_mut(container.parent).unwrap();
            container.items.retain(|v| v != &entity);
            commands.entity(entity)
                .insert(Holder { held_by: character })
                .remove::<ParentContainer>();
        } else if let Some(equipped) = equipped {
            let mut equipped_character = character_equipment.get_mut(equipped.parent).unwrap();
            equipped_character.equipment.retain(|e| e.entity != entity);
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
    clients: Query<(&NetClient, &Possessing)>,
    characters: Query<(&Location, &Held)>,
    mut containers: Query<&mut Container>,
    mut commands: Commands,
) {
    for event in events.read() {
        let (client, owner) = match clients.get(event.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let character = owner.entity;
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
                    .insert(Location {
                        position: character_position.position,
                        map_id: character_position.map_id,
                        ..Default::default()
                    });
            }
        } else {
            commands.entity(target)
                .remove::<Holder>()
                .insert(Location {
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
    clients: Query<(&NetClient, &Possessing)>,
    characters: Query<(&Location, &Held)>,
    mut loadouts: Query<&mut Character>,
    mut commands: Commands,
) {
    for event in events.read() {
        let (client, owner) = match clients.get(event.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let character = owner.entity;
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
            target_character.equipment.push(CharacterEquipped {
                entity: target,
                slot: event.slot,
            });
            commands.entity(target)
                .remove::<Holder>()
                .insert(EquippedBy {
                    parent: event.character,
                    slot: event.slot,
                });
        } else {
            commands.entity(target)
                .remove::<Holder>()
                .insert(Location {
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
    clients: Query<(&NetClient, &Possessing)>,
    characters: Query<&NetEntity>,
    mut context_requests: Query<&mut ContextMenuRequest>,
    mut context_events: EventReader<ContextMenuEvent>,
) {
    for mut request in context_requests.iter_mut() {
        request.entries.push(ContextMenuEntry {
            id: 0,
            text_id: 3000489,
            hue: None,
            flags: Default::default(),
        });
    }

    for ContextMenuEvent { client_entity, target, .. } in context_events.read() {
        let (client, owned) = match clients.get(*client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let net = match characters.get(owned.entity) {
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
    for request in requests.read() {
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
            profile: "Bio".to_string(),
        }).into());
    }
}

pub fn handle_skills_requests(
    clients: Query<&NetClient>,
    mut requests: EventReader<RequestSkillsEvent>,
) {
    for request in requests.read() {
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
            ],
        }).into());
    }
}

pub fn handle_war_mode(
    mut commands: Commands,
    clients: Query<(&NetClient, &Possessing)>,
    mut characters: Query<&mut Flags>,
    mut new_packets: EventReader<ReceivedPacketEvent>,
) {
    for ReceivedPacketEvent { client_entity, packet } in new_packets.read() {
        let packet = match packet.downcast::<WarMode>() {
            Some(x) => x,
            _ => continue,
        };

        let (client, owned) = match clients.get(*client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let mut flags = match characters.get_mut(owned.entity) {
            Ok(x) => x,
            _ => continue,
        };

        if packet.war {
            flags.flags |= EntityFlags::WAR_MODE;
        } else {
            flags.flags &= !EntityFlags::WAR_MODE;
            commands.entity(owned.entity).remove::<AttackTarget>();
        }

        client.send_packet(packet.clone().into());
    }
}
