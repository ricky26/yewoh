use bevy::prelude::*;
use glam::ivec2;
use bevy::ecs::entity::{EntityHashMap, EntityHashSet};
use yewoh::protocol::{AnyPacket, BeginEnterWorld, ChangeSeason, EndEnterWorld, ExtendedCommand};
use yewoh::protocol::{CharacterEquipment, OpenContainer, UpsertContainerContents};

use crate::world::characters::{CharacterBodyType, CharacterQuery};
use crate::world::connection::{NetClient, OwningClient, Possessing};
use crate::world::delta_grid::{delta_grid_cell, Delta, DeltaEntry, DeltaGrid};
use crate::world::entity::{ContainedPosition, EquippedPosition, MapPosition};
use crate::world::items::{Container, ContainerOpenedEvent, ItemQuery};
use crate::world::map::MapInfos;
use crate::world::net_id::NetId;
use crate::world::ServerSet;
use crate::world::spatial::SpatialQuery;

pub const DEFAULT_VIEW_RANGE: i32 = 18;

#[derive(Debug, Clone, Reflect, Component)]
#[reflect(Component)]
#[require(SeenEntities)]
pub struct View {
    pub range: i32,
}

impl Default for View {
    fn default() -> Self {
        View { range: DEFAULT_VIEW_RANGE }
    }
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
pub struct Synchronizing;

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
pub struct Synchronized;

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
pub struct EnteredWorld;

#[derive(Debug, Clone, PartialEq, Eq, Component, Reflect)]
#[reflect(Component)]
pub struct ViewKey {
    pub possessing: Entity,
    pub map_id: u8,
}

impl FromWorld for ViewKey {
    fn from_world(_world: &mut World) -> Self {
        ViewKey {
            possessing: Entity::PLACEHOLDER,
            map_id: 0,
        }
    }
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct SeenEntities {
    pub seen_entities: EntityHashSet,
    pub open_containers: EntityHashSet,
    pub parents: EntityHashMap<Entity>,
    pub children: EntityHashMap<EntityHashSet>,
}

impl SeenEntities {
    fn children_mut(&mut self, entity: Entity) -> &mut EntityHashSet {
        self.children.entry(entity).or_default()
    }

    pub fn insert_entity(&mut self, entity: Entity, parent: Option<Entity>) {
        self.seen_entities.insert(entity);

        let existing_parent = self.parents.get(&entity).copied();
        if existing_parent == parent {
            return;
        }

        if let Some(parent) = existing_parent {
            let children = self.children_mut(parent);
            children.remove(&entity);
            if children.is_empty() {
                self.children.remove(&parent);
            }
        }

        if let Some(parent) = parent {
            self.parents.insert(entity, parent);
            self.children_mut(parent).insert(entity);
        }
    }

    pub fn remove_entity(&mut self, entity: Entity) {
        self.seen_entities.remove(&entity);
        self.open_containers.remove(&entity);
        self.parents.remove(&entity);

        if let Some(children) = self.children.remove(&entity) {
            for child in children {
                self.remove_entity(child);
            }
        }
    }
}

fn view_aabb(center: IVec2, range: i32) -> (IVec2, IVec2) {
    let range2 = IVec2::splat(range.abs());
    let min = center - range2;
    let max = center + range2;
    (min, max)
}

pub fn send_deltas(
    delta_grid: Res<DeltaGrid>,
    mut clients: Query<(&NetClient, &View, &mut SeenEntities, &Possessing), With<Synchronized>>,
    owned: Query<&MapPosition, With<OwningClient>>,
    mut deltas: Local<Vec<Delta>>,
    character_query: Query<(&NetId, CharacterQuery, Option<&Children>)>,
    equipment_query: Query<(&NetId, ItemQuery), With<EquippedPosition>>,
) {
    for (client, view, mut seen, possessing) in &mut clients {
        let location = match owned.get(possessing.entity) {
            Ok(x) => *x,
            _ => continue,
        };

        let Some(delta_map) = delta_grid.maps.get(&location.map_id) else {
            continue;
        };

        let (min, max) = view_aabb(location.position.truncate(), view.range);
        let grid_min = delta_grid_cell(min);
        let grid_max = delta_grid_cell(max);
        for grid_x in grid_min.x..=grid_max.x {
            for grid_y in grid_min.y..=grid_max.y {
                let grid_pos = ivec2(grid_x, grid_y);
                let Some(cell) = delta_map.cell_at(grid_pos) else {
                    continue;
                };

                deltas.extend(cell.deltas.iter().cloned());
            }
        }

        deltas.sort_by_key(|d| d.version);

        let mut last_version = None;
        for delta in deltas.drain(..) {
            if Some(delta.version) == last_version {
                continue;
            }

            match delta.entry {
                DeltaEntry::ItemChanged { entity, parent, packet } => {
                    if seen.seen_entities.contains(&entity) {
                        seen.insert_entity(entity, parent);
                        client.send_packet_arc(packet);
                    } else if let Some(parent) = parent {
                        if seen.open_containers.contains(&parent) {
                            seen.insert_entity(entity, Some(parent));
                            client.send_packet_arc(packet);
                        }
                    } else {
                        seen.insert_entity(entity, None);
                        client.send_packet_arc(packet);
                    }
                }
                DeltaEntry::ItemRemoved { entity, packet, .. } => {
                    seen.remove_entity(entity);
                    client.send_packet_arc(packet);
                }
                DeltaEntry::CharacterChanged { entity, update_packet, .. } => {
                    if seen.seen_entities.contains(&entity) {
                        client.send_packet_arc(update_packet);
                    } else {
                        let Ok((id, character, children)) = character_query.get(entity) else {
                            continue;
                        };

                        let mut equipment = Vec::new();
                        if let Some(children) = children {
                            equipment.reserve(children.len());

                            for child in children {
                                let Ok((child_id, item)) = equipment_query.get(*child) else {
                                    continue;
                                };

                                let equipped = item.position.equipped.as_ref().unwrap();
                                equipment.push(CharacterEquipment {
                                    id: child_id.id,
                                    graphic_id: **item.graphic,
                                    slot: equipped.slot,
                                    hue: **item.hue,
                                });
                            }
                        }

                        let packet = character.to_upsert(id.id, equipment);
                        client.send_packet(AnyPacket::from_packet(packet));
                        seen.insert_entity(entity, None);
                        seen.open_containers.insert(entity);
                    }
                }
                DeltaEntry::CharacterRemoved { entity, packet, .. } => {
                    seen.remove_entity(entity);
                    client.send_packet_arc(packet);
                }
                DeltaEntry::CharacterAnimation { entity, packet } => {
                    if seen.seen_entities.contains(&entity) {
                        client.send_packet_arc(packet);
                    }
                }
                DeltaEntry::CharacterDamaged { entity, packet } => {
                    if seen.seen_entities.contains(&entity) {
                        client.send_packet_arc(packet);
                    }
                }
                DeltaEntry::CharacterSwing { entity, packet, .. } => {
                    if seen.seen_entities.contains(&entity) {
                        client.send_packet_arc(packet);
                    }
                }
                DeltaEntry::CharacterStatusChanged { entity, packet } => {
                    if entity != possessing.entity && seen.seen_entities.contains(&entity) {
                        client.send_packet_arc(packet);
                    }
                }
                DeltaEntry::TooltipChanged { entity, packet, .. } => {
                    if seen.seen_entities.contains(&entity) {
                        client.send_packet_arc(packet);
                    }
                }
            }

            last_version = Some(delta.version);
        }
    }
}

pub fn sync_visible_entities(
    spatial_query: SpatialQuery,
    mut clients: Query<(&NetClient, &View, &mut SeenEntities, &Possessing), With<Synchronizing>>,
    owned: Query<&MapPosition, With<OwningClient>>,
    character_query: Query<(&NetId, CharacterQuery, Option<&Children>)>,
    item_query: Query<(&NetId, ItemQuery)>,
) {
    for (client, view, mut seen, possessing) in &mut clients {
        let location = match owned.get(possessing.entity) {
            Ok(x) => *x,
            _ => continue,
        };

        let map_characters = spatial_query.characters.lookup.maps.get(&location.map_id);
        let map_items = spatial_query.dynamic_items.lookup.maps.get(&location.map_id);

        let (min, max) = view_aabb(location.position.truncate(), view.range);
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                let test_pos = ivec2(x, y);

                if let Some(characters) = map_characters {
                    for entry in characters.entries_at(test_pos) {
                        let Ok((id, character, children)) = character_query.get(entry.entity) else {
                            continue;
                        };

                        seen.insert_entity(entry.entity, None);
                        seen.open_containers.insert(entry.entity);

                        let mut equipment = Vec::new();
                        if let Some(children) = children {
                            equipment.reserve(children.len());

                            for child in children {
                                let Ok((child_id, item)) = item_query.get(*child) else {
                                    continue;
                                };

                                let Some(equipped) = item.position.equipped.as_ref() else {
                                    continue;
                                };

                                seen.insert_entity(*child, Some(entry.entity));
                                equipment.push(CharacterEquipment {
                                    id: child_id.id,
                                    graphic_id: **item.graphic,
                                    slot: equipped.slot,
                                    hue: **item.hue,
                                });
                            }
                        }

                        let packet = character.to_upsert(id.id, equipment);
                        client.send_packet(AnyPacket::from_packet(packet));

                        let packet = if entry.entity == possessing.entity {
                            character.to_full_status_packet(id.id)
                        } else {
                            character.to_status_packet(id.id)
                        };
                        client.send_packet(AnyPacket::from_packet(packet));
                    }
                }

                if let Some(items) = map_items {
                    for entry in items.entries_at(test_pos) {
                        let Ok((id, item)) = item_query.get(entry.entity) else {
                            continue;
                        };

                        let Some(packet) = item.to_upsert(id.id, None) else {
                            continue;
                        };

                        seen.insert_entity(entry.entity, None);
                        client.send_packet(packet);
                    }
                }
            }
        }
    }
}

pub fn start_synchronizing(
    clients: Query<
        (Entity, Option<&ViewKey>, Ref<Possessing>),
        (With<NetClient>, Without<Synchronizing>),
    >,
    characters: Query<&MapPosition, (With<OwningClient>, With<NetId>, With<MapPosition>, With<CharacterBodyType>)>,
    mut commands: Commands,
) {
    for (entity, view_key, possessing) in &clients {
        let map_position = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        let new_view_key = ViewKey { possessing: possessing.entity, map_id: map_position.map_id };
        if view_key == Some(&new_view_key) {
            continue;
        }

        commands.entity(entity)
            .remove::<Synchronized>()
            .insert((
                Synchronizing,
                new_view_key,
            ));
    }
}

pub fn send_change_map(
    mut clients: Query<(&NetClient, &mut SeenEntities, Ref<Possessing>), With<Synchronizing>>,
    characters: Query<(&NetId, &MapPosition, Ref<CharacterBodyType>)>,
    maps: Res<MapInfos>,
) {
    for (client, mut seen_entities, possessing) in clients.iter_mut() {
        let (possessed_net, map_position, body_type) = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        let map = match maps.maps.get(&map_position.map_id) {
            Some(v) => v,
            None => continue,
        };

        seen_entities.seen_entities.clear();
        seen_entities.open_containers.clear();

        client.send_packet(BeginEnterWorld {
            entity_id: possessed_net.id,
            body_type: **body_type,
            position: map_position.position,
            direction: map_position.direction,
            map_size: map.size,
        }.into());
        client.send_packet(ExtendedCommand::ChangeMap(map_position.map_id).into());
        client.send_packet(ChangeSeason { season: map.season, play_sound: true }.into());
    }
}

pub fn finish_synchronizing(
    clients: Query<(Entity, &NetClient, Option<&EnteredWorld>), With<Synchronizing>>,
    mut commands: Commands,
) {
    for (entity, client, entered_world) in clients.iter() {
        commands.entity(entity)
            .remove::<Synchronizing>()
            .insert(Synchronized)
            .insert(EnteredWorld);

        if entered_world.is_none() {
            client.send_packet(EndEnterWorld.into());
        }
    }
}

pub fn send_opened_containers(
    net_ids: Query<&NetId>,
    mut clients: Query<(&NetClient, &mut SeenEntities)>,
    mut events: EventReader<ContainerOpenedEvent>,
    containers: Query<(&Container, Option<&Children>)>,
    contained_items: Query<(Entity, &NetId, ItemQuery), (With<Parent>, With<ContainedPosition>)>,
) {
    for event in events.read() {
        let (client, mut seen) = match clients.get_mut(event.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let id = match net_ids.get(event.container) {
            Ok(x) => x.id,
            _ => continue,
        };

        let Ok((container, children)) = containers.get(event.container) else {
            continue;
        };

        seen.open_containers.insert(event.container);

        let mut contents = Vec::new();
        if let Some(children) = children {
            contents.reserve(children.len());

            for child in children {
                let Ok((child, child_id, item)) = contained_items.get(*child) else {
                    continue;
                };
                seen.insert_entity(child, Some(event.container));
                contents.push(item.to_upsert_contained(child_id.id, id).unwrap());
            }
        }

        client.send_packet(OpenContainer {
            id,
            gump_id: container.gump_id,
        }.into());
        client.send_packet(UpsertContainerContents {
            contents,
        }.into());
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<OwningClient>()
        .add_systems(First, (
            start_synchronizing,
        ).in_set(ServerSet::HandlePackets))
        .add_systems(Last, (
            send_change_map,
        ).in_set(ServerSet::SendFirst))
        .add_systems(Last, (
            send_deltas,
            sync_visible_entities,
        ).in_set(ServerSet::SendEntities))
        .add_systems(Last, (
            send_opened_containers,
            finish_synchronizing,
        ).in_set(ServerSet::Send));
}
