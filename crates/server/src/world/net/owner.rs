use std::collections::HashMap;

use bevy_ecs::prelude::*;
use glam::UVec2;

use yewoh::protocol::{BeginEnterWorld, ChangeSeason, EndEnterWorld, ExtendedCommand, SetTime};

use crate::world::entity::{Character, MapPosition};
use crate::world::events::NewPrimaryEntityEvent;
use crate::world::net::connection::NetClient;
use crate::world::net::entity::NetEntity;

#[derive(Debug, Clone, Component)]
pub struct NetOwned {
    pub primary_entity: Entity,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct NetOwner {
    pub client: Entity,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct NetSynchronizing;

#[derive(Debug, Clone, Copy, Component)]
pub struct NetSynchronized;

#[derive(Debug, Clone)]
pub struct MapInfo {
    pub size: UVec2,
    pub season: u8,
}

#[derive(Debug, Clone, Default)]
pub struct MapInfos {
    pub maps: HashMap<u8, MapInfo>,
}

pub fn start_synchronizing(
    maps: Res<MapInfos>,
    clients: Query<(&NetClient, Option<&NetOwned>)>,
    characters: Query<(&NetEntity, &MapPosition, &Character)>,
    mut events: EventReader<NewPrimaryEntityEvent>,
    mut commands: Commands,
) {
    for event in events.iter() {
        let entity = event.client;
        let (client, owned) = match clients.get(entity) {
            Ok(x) => x,
            _ => continue,
        };

        let old_primary_entity = owned.map(|o| o.primary_entity);
        if old_primary_entity == event.primary_entity {
            continue;
        }

        if let Some(old_primary) = old_primary_entity {
            commands.entity(old_primary).remove::<NetOwner>();
        }

        let primary_entity = match event.primary_entity {
            Some(x) => x,
            None => {
                commands.entity(entity).remove::<NetOwned>();
                continue;
            }
        };

        commands.entity(primary_entity).insert(NetOwner { client: entity });
        commands.entity(entity)
            .insert(NetOwned { primary_entity })
            .remove::<NetSynchronized>()
            .insert(NetSynchronizing);

        let (primary_net, map_position, character) = match characters.get(primary_entity) {
            Ok(x) => x,
            Err(_) => {
                continue;
            }
        };

        let MapPosition { position, map_id, direction } = map_position.clone();
        let map = match maps.maps.get(&map_id) {
            Some(v) => v,
            None => continue,
        };

        let entity_id = primary_net.id;
        let body_type = character.body_type;

        client.send_packet(BeginEnterWorld {
            entity_id,
            body_type,
            position,
            direction,
            map_size: map.size,
        }.into());
        client.send_packet(ExtendedCommand::ChangeMap(map_id).into());
        client.send_packet(ChangeSeason { season: map.season, play_sound: true }.into());
    }
}

pub fn finish_synchronizing(
    clients: Query<(Entity, &NetClient), With<NetSynchronizing>>,
    mut commands: Commands,
) {
    for (entity, client) in clients.iter() {
        commands.entity(entity)
            .remove::<NetSynchronizing>()
            .insert(NetSynchronized);
        client.send_packet(EndEnterWorld.into());
        client.send_packet(SetTime {
            hour: 12,
            minute: 16,
            second: 31,
        }.into());
    }
}
