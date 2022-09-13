use bevy_ecs::prelude::*;

use yewoh::protocol::{MoveConfirm, MoveEntityReject, OpenContainer, OpenPaperDoll};
use yewoh_server::world::entity::{Character, Container, EquippedBy, Graphic, MapPosition, Notorious, ParentContainer, Quantity};
use yewoh_server::world::events::{DoubleClickEvent, DropEvent, EquipEvent, MoveEvent, PickUpEvent};
use yewoh_server::world::net::{make_container_contents_packet, NetClient, NetEntity, NetOwned, PlayerState};

#[derive(Debug, Clone, Component)]
pub struct Held {
    pub held_entity: Entity,
}

#[derive(Debug, Clone, Component)]
pub struct Holder {
    pub held_by: Entity,
}

pub fn handle_move(
    mut events: EventReader<MoveEvent>,
    connection_query: Query<(&NetClient, &NetOwned)>,
    mut character_query: Query<(&mut MapPosition, &mut PlayerState, &Notorious)>,
) {
    for MoveEvent { client: connection, request } in events.iter() {
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
            map_position.position += request.direction.as_vec2().extend(0);
        }

        state.position = *map_position;

        let notoriety = **notoriety;
        client.send_packet(MoveConfirm {
            sequence: request.sequence,
            notoriety,
        }.into());
    }
}

pub fn handle_double_click(
    mut events: EventReader<DoubleClickEvent>,
    clients: Query<&NetClient>,
    target_query: Query<(&NetEntity, Option<&Character>, Option<&Container>)>,
    content_query: Query<(&NetEntity, &ParentContainer, &Graphic, Option<&Quantity>)>,
) {
    for DoubleClickEvent { client, target } in events.iter() {
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
        let (client, owner) = match clients.get(event.client) {
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
        let (client, owner) = match clients.get(event.client) {
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
        let (client, owner) = match clients.get(event.client) {
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
