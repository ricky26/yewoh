use bevy_ecs::prelude::*;

use yewoh::{EntityId, EntityKind};
use yewoh::protocol::{CharacterEquipment, DeleteEntity, EntityFlags, Packet, UpsertContainerContents, UpsertEntityCharacter, UpsertEntityContained, UpsertEntityEquipped, UpsertEntityWorld, UpsertLocalPlayer};

use crate::world::entity::{Character, Container, EquippedBy, Graphic, KnownPosition, MapPosition, Notorious, ParentContainer, Quantity, Stats};
use crate::world::net::{broadcast, NetClient, NetEntity, NetEntityLookup, NetOwner};

pub fn send_player_updates(
    clients: Query<&NetClient>,
    query: Query<
        (&NetOwner, &NetEntity, &Character, &MapPosition),
        Or<(Changed<Character>, Changed<MapPosition>)>,
    >,
) {
    for (owner, entity, character, position) in query.iter() {
        if let Ok(client) = clients.get(owner.client) {
            client.send_packet(UpsertLocalPlayer {
                id: entity.id,
                body_type: character.body_type,
                server_id: 0,
                hue: character.hue,
                flags: EntityFlags::empty(),
                position: position.position,
                direction: position.direction,
            }.into());
        }
    }
}

pub fn send_entity_updates(
    lookup: Res<NetEntityLookup>,
    clients: Query<(Entity, &NetClient)>,
    world_items_query: Query<
        (&NetEntity, &Graphic, Option<&Quantity>, &MapPosition),
        Or<(Changed<Graphic>, Changed<MapPosition>)>,
    >,
    characters_query: Query<
        (&NetEntity, &Character, &MapPosition, &Notorious, Option<&NetOwner>, Option<&KnownPosition>),
        Or<(Changed<Character>, Changed<MapPosition>, Changed<Notorious>)>,
    >,
    equipment_query: Query<(&NetEntity, &Graphic, &EquippedBy), Or<(Changed<Graphic>, Changed<EquippedBy>)>>,
    content_query: Query<
        (&NetEntity, &ParentContainer, &Graphic, Option<&Quantity>),
        Or<(Changed<Graphic>, Changed<Quantity>, Changed<ParentContainer>)>,
    >,
    all_equipment_query: Query<(&NetEntity, &Graphic, &EquippedBy)>,
) {
    // TODO: implement an interest system

    for (net, graphic, quantity, position) in world_items_query.iter() {
        let id = net.id;
        let packet = UpsertEntityWorld {
            id,
            kind: EntityKind::Item,
            graphic_id: graphic.id,
            graphic_inc: 0,
            direction: position.direction,
            quantity: quantity.map_or(1, |q| q.quantity),
            position: position.position,
            hue: graphic.hue,
            flags: Default::default(),
        }.into_arc();

        broadcast(clients.iter().map(|(_, c)| c), packet.clone());
    }

    for (net, character, position, notoriety, owner, known_position) in characters_query.iter() {
        let entity_id = net.id;
        let hue = character.hue;
        let body_type = character.body_type;
        let mut equipment = Vec::new();

        for child_entity in character.equipment.iter().copied() {
            let (net, graphic, equipped_by) = match all_equipment_query.get(child_entity) {
                Ok(x) => x,
                _ => continue,
            };
            equipment.push(CharacterEquipment {
                id: net.id,
                slot: equipped_by.slot,
                graphic_id: graphic.id,
                hue: graphic.hue,
            });
        }

        let packet = UpsertEntityCharacter {
            id: entity_id,
            body_type,
            position: position.position,
            direction: position.direction,
            hue,
            flags: EntityFlags::empty(),
            notoriety: notoriety.0,
            equipment,
        }.into_arc();

        for (entity, client) in clients.iter() {
            if Some(entity) == owner.map(|x| x.client)
                && known_position.map_or(false, |x| x.expected_position == Some(*position)) {
                continue;
            }

            client.send_packet_arc(packet.clone());
        }
    }

    for (net, graphic, equipped_by) in equipment_query.iter() {
        let parent_id = match lookup.ecs_to_net(equipped_by.entity) {
            Some(x) => x,
            None => continue,
        };

        broadcast(clients.iter().map(|(_, c)| c), UpsertEntityEquipped {
            id: net.id,
            parent_id,
            slot: equipped_by.slot,
            graphic_id: graphic.id,
            hue: graphic.hue,
        }.into_arc());
    }

    for (net, parent, graphic, quantity) in content_query.iter() {
        let parent_id = match lookup.ecs_to_net(parent.container) {
            Some(x) => x,
            None => continue,
        };

        broadcast(clients.iter().map(|(_, c)| c), UpsertEntityContained {
            id: net.id,
            parent_id,
            graphic_id: graphic.id,
            graphic_inc: 0,
            quantity: quantity.map_or(1, |q| q.quantity),
            position: parent.position,
            hue: graphic.hue,
            grid_index: parent.grid_index,
        }.into_arc());
    }
}

pub fn send_updated_container_contents(
    clients: Query<&NetClient>,
    container_query: Query<(&NetEntity, &Container), Changed<Container>>,
    content_query: Query<(&NetEntity, &ParentContainer, &Graphic, Option<&Quantity>)>,
) {
    for (net, container) in container_query.iter() {
        broadcast(clients.iter(), make_container_contents_packet(net.id, container, &content_query).into_arc());
    }
}

pub fn make_container_contents_packet(
    id: EntityId, container: &Container,
    content_query: &Query<(&NetEntity, &ParentContainer, &Graphic, Option<&Quantity>)>,
) -> UpsertContainerContents {
    let mut items = Vec::with_capacity(container.items.len());

    for item in container.items.iter() {
        let item = *item;
        let (net_id, parent, graphic, quantity) = match content_query.get(item) {
            Ok(x) => x,
            _ => continue,
        };

        items.push(UpsertEntityContained {
            id: net_id.id,
            graphic_id: graphic.id,
            graphic_inc: 0,
            quantity: quantity.map_or(1, |q| q.quantity),
            position: parent.position,
            grid_index: parent.grid_index,
            parent_id: id,
            hue: graphic.hue,
        });
    }

    UpsertContainerContents {
        items,
    }
}

pub fn send_remove_entity(
    lookup: Res<NetEntityLookup>,
    clients: Query<&NetClient>,
    removals: RemovedComponents<NetEntity>,
) {
    for entity in removals.iter() {
        let id = match lookup.ecs_to_net(entity) {
            Some(x) => x,
            None => continue,
        };

        broadcast(clients.iter(), DeleteEntity { id }.into_arc());
    }
}

pub fn send_updated_stats(
    clients: Query<&NetClient>,
    query: Query<(&NetEntity, &Stats), Changed<Stats>>,
) {
    for (net, stats) in query.iter() {
        broadcast(clients.iter(), stats.upsert(net.id, true).into_arc());
    }
}
