use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use glam::{IVec3, UVec2};

use yewoh::protocol::{BeginEnterWorld, ChangeSeason, EndEnterWorld, ExtendedCommand};

use crate::world::entity::{Character, MapPosition};
use crate::world::events::NewPrimaryEntityEvent;
use crate::world::net::connection::NetClient;
use crate::world::net::entity::NetEntity;

#[derive(Debug, Clone, Component, Reflect)]
pub struct NetOwned {
    pub primary_entity: Entity,
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct NetOwner {
    pub client_entity: Entity,
}

#[derive(Debug, Clone, Component, Reflect)]
pub struct View {
    pub map_id: u8,
    pub position: IVec3,
}

#[derive(Debug, Clone, Component)]
pub struct CanSee {
    pub entities: HashSet<Entity>,
}

#[derive(Debug, Clone, Component)]
pub struct HasSeen {
    pub entities: HashSet<Entity>,
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct NetSynchronizing;

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct NetSynchronized;

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct EnteredWorld;

#[derive(Debug, Clone)]
pub struct MapInfo {
    pub size: UVec2,
    pub season: u8,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Default, Resource)]
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
        let client_entity = event.client_entity;
        let (client, owned) = match clients.get(client_entity) {
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
                commands.entity(client_entity).remove::<NetOwned>();
                continue;
            }
        };

        commands.entity(primary_entity).insert(NetOwner { client_entity });
        commands.entity(client_entity)
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

        commands.entity(client_entity)
            .insert(View { map_id, position });
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

pub fn update_view(
    maps: Res<MapInfos>,
    mut clients: Query<(Entity, &NetClient, &mut View)>,
    characters: Query<(&NetOwner, &MapPosition), Changed<MapPosition>>,
    mut commands: Commands,
) {
    for (owner, MapPosition { map_id, position, .. }) in characters.iter() {
        let (entity, client, mut view) = match clients.get_mut(owner.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let map_id = *map_id;
        view.position = *position;
        if view.map_id == map_id {
            continue;
        }

        view.map_id = map_id;
        let map = match maps.maps.get(&map_id) {
            Some(v) => v,
            None => continue,
        };

        commands.entity(entity)
            .remove::<NetSynchronized>()
            .insert(NetSynchronizing);
        client.send_packet(ExtendedCommand::ChangeMap(map_id).into());
        client.send_packet(ChangeSeason { season: map.season, play_sound: true }.into());
    }
}

pub fn finish_synchronizing(
    clients: Query<(Entity, &NetClient, Option<&EnteredWorld>), With<NetSynchronizing>>,
    mut commands: Commands,
) {
    for (entity, client, entered_world) in clients.iter() {
        commands.entity(entity)
            .remove::<NetSynchronizing>()
            .insert(NetSynchronized)
            .insert(EnteredWorld);

        if entered_world.is_none() {
            client.send_packet(EndEnterWorld.into());
        }
    }
}
