use std::fmt::Debug;

use bevy::prelude::*;
use glam::ivec2;
use bevy::ecs::entity::{EntityHashMap, EntityHashSet};
use smallvec::{smallvec, SmallVec};
use yewoh::EntityId;
use yewoh::protocol::{BeginEnterWorld, ChangeSeason, DeleteEntity, EndEnterWorld, ExtendedCommand};
use yewoh::protocol::{CharacterEquipment, OpenContainer, UpsertContainerContents};

use crate::world::characters::{CharacterBodyType, CharacterQuery};
use crate::world::connection::{NetClient, OwningClient, Possessing};
use crate::world::delta_grid::{delta_grid_cell, Delta, DeltaEntry, DeltaGrid};
use crate::world::entity::{ContainedPosition, EquippedPosition, MapPosition, RootPosition};
use crate::world::items::{Container, OnContainerOpen, ItemQuery, ItemGraphic};
use crate::world::map::MapInfos;
use crate::world::net_id::NetId;
use crate::world::ServerSet;
use crate::world::spatial::SpatialQuery;

pub const DEFAULT_VIEW_RANGE: i32 = 18;
pub const MIN_VIEW_RANGE: i32 = 5;
pub const MAX_VIEW_RANGE: i32 = 24;

#[derive(Debug, Clone, Reflect, Component)]
#[reflect(Component)]
#[require(SeenEntities, LastView)]
pub struct View {
    pub range: i32,
}

impl Default for View {
    fn default() -> Self {
        View { range: DEFAULT_VIEW_RANGE }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Default)]
pub struct ViewRect {
    pub min: IVec2,
    pub max: IVec2,
}

impl ViewRect {
    pub fn is_empty(&self) -> bool {
        let delta = self.max - self.min;
        (delta.x < 0) || (delta.y < 0)
    }

    pub fn from_range(position: IVec2, range: i32) -> ViewRect {
        let delta = IVec2::splat(range);
        let min = position - delta;
        let max = position + delta;
        ViewRect { min, max }
    }

    #[inline]
    fn value_intersects(a_min: i32, a_max: i32, b_min: i32, b_max: i32) -> bool {
        (a_min < b_max) && (b_min < a_max)
    }

    pub fn contains(&self, point: IVec2) -> bool {
        (point.x >= self.min.x) &&
            (point.x < self.max.x) &&
            (point.y >= self.min.y) &&
            (point.y < self.max.y)
    }

    pub fn intersects(&self, other: &ViewRect) -> bool {
        Self::value_intersects(self.min.x, self.max.x, other.min.x, other.max.x) &&
            Self::value_intersects(self.min.y, self.max.y, other.min.y, other.max.y)
    }

    pub fn iter_exposed(&self, old: &ViewRect) -> impl Iterator<Item = IVec2> + Clone + Debug {
        if self == old || self.is_empty() {
            smallvec![].into_iter().flatten()
        } else if old.is_empty() || !self.intersects(old) {
            smallvec![RectIter::from_rect(*self)].into_iter().flatten()
        } else {
            let mut parts: SmallVec<[RectIter; 4]> = SmallVec::new();

            let top_y = old.min.y.max(self.min.y);
            let bottom_y = old.max.y.min(self.max.y);

            let top = ViewRect {
                min: self.min,
                max: ivec2(self.max.x, top_y),
            };
            if !top.is_empty() {
                parts.push(RectIter::from_rect(top));
            }

            let bottom = ViewRect {
                min: ivec2(self.min.x, bottom_y),
                max: self.max,
            };
            if !bottom.is_empty() {
                parts.push(RectIter::from_rect(bottom));
            }

            let left = ViewRect {
                min: ivec2(self.min.x, top_y),
                max: ivec2(old.min.x, bottom_y),
            };
            if !left.is_empty() {
                parts.push(RectIter::from_rect(left));
            }

            let right = ViewRect {
                min: ivec2(old.max.x, top_y),
                max: ivec2(self.max.x, bottom_y),
            };
            if !right.is_empty() {
                parts.push(RectIter::from_rect(right));
            }

            parts.into_iter().flatten()
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RectIter {
    min: IVec2,
    width: i32,
    len: usize,
    offset: usize,
}

impl RectIter {
    fn from_min_max(min: IVec2, max: IVec2) -> RectIter {
        let delta = max - min;
        let width = delta.x.max(0);
        let len = (width * delta.y.max(0)) as usize;
        RectIter { min, width, len, offset: 0 }
    }

    fn from_rect(rect: ViewRect) -> RectIter {
        Self::from_min_max(rect.min, rect.max)
    }

    fn get(&self, offset: usize) -> IVec2 {
        let offset = offset as i32;
        let y = offset / self.width;
        let x = offset % self.width;
        self.min + ivec2(x, y)
    }
}

impl Iterator for RectIter {
    type Item = IVec2;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.len {
            let value = self.get(self.offset);
            self.offset += 1;
            Some(value)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Default, Reflect, Component)]
#[reflect(Default, Component)]
pub struct LastView(pub ViewRect);

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
pub struct Synchronizing;

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
pub struct Synchronized;

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
pub struct StartedEnteringWorld;

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

#[derive(Clone, Debug, Default)]
struct SeenEntity {
    parent: Option<Entity>,
    children: EntityHashSet,
    see_inside: bool,
    id: EntityId,
    position: IVec2,
}

#[derive(Debug, Clone, Default, Component)]
pub struct SeenEntities {
    seen_entities: EntityHashMap<SeenEntity>,
}

impl SeenEntities {
    pub fn insert_entity(
        &mut self, entity: Entity, parent: Option<Entity>, id: EntityId, position: IVec2,
    ) {
        let seen = self.seen_entities.entry(entity).or_insert_with(|| default());
        seen.id = id;
        seen.position = position;
        if seen.parent == parent {
            return;
        }

        let existing_parent = std::mem::replace(&mut seen.parent, parent);

        if let Some(parent) = existing_parent {
            let parent_mut = self.seen_entities.get_mut(&parent).unwrap();
            parent_mut.children.remove(&entity);
        }

        if let Some(parent) = parent {
            let parent_mut = self.seen_entities.get_mut(&parent).unwrap();
            parent_mut.children.insert(entity);
        }
    }

    pub fn remove_entity(&mut self, entity: Entity) -> bool {
         if let Some(mut seen) = self.seen_entities.remove(&entity) {
             for child in seen.children.drain() {
                 self.remove_entity(child);
             }
             true
         } else {
             false
         }
    }

    pub fn retain(&mut self, mut f: impl FnMut(Entity, EntityId, IVec2) -> bool) {
        let mut children_to_cleanup = Vec::new();

        self.seen_entities.retain(|entity, seen| {
            if !f(*entity, seen.id, seen.position) {
                children_to_cleanup.extend(seen.children.drain());
                false
            } else {
                true
            }
        });

        for child in children_to_cleanup {
            self.remove_entity(child);
        }
    }

    pub fn insert_entity_if_visible(
        &mut self, entity: Entity, parent: Option<Entity>, id: EntityId, position: IVec2,
    ) -> bool {
        if let Some(parent) = parent {
            if !self.can_see_inside(parent) && !self.seen_entities.contains_key(&entity) {
                return false;
            }
        }

        self.insert_entity(entity, parent, id, position);
        true
    }

    pub fn clear(&mut self) {
        self.seen_entities.clear();
    }

    pub fn open_container(&mut self, entity: Entity) {
        if let Some(seen) = self.seen_entities.get_mut(&entity) {
            seen.see_inside = true;
        }
    }

    pub fn has_seen(&self, entity: Entity) -> bool {
        self.seen_entities.contains_key(&entity)
    }

    pub fn can_see_inside(&self, entity: Entity) -> bool {
        self.seen_entities.get(&entity).map_or(false, |e| e.see_inside)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Component, Reflect)]
#[reflect(Component)]
pub struct ExpectedCharacterState {
    pub body_type: u16,
    pub hue: u16,
    pub flags: u8,
    pub position: MapPosition,
}

fn view_aabb(center: IVec2, range: i32) -> ViewRect {
    let range2 = IVec2::splat(range.abs());
    let min = center - range2;
    let max = center + range2;
    ViewRect { min, max }
}

pub fn send_deltas(
    mut commands: Commands,
    delta_grid: Res<DeltaGrid>,
    mut clients: Query<
        (
            Entity,
            &NetClient,
            &ViewKey,
            &LastView,
            &mut SeenEntities,
            &Possessing,
            Option<&ExpectedCharacterState>,
        ),
        With<Synchronized>,
    >,
    owned: Query<(&NetId, CharacterQuery), With<OwningClient>>,
    mut deltas: Local<Vec<Delta>>,
    item_query: Query<&NetId, With<ItemGraphic>>,
    character_query: Query<(&NetId, CharacterQuery, Option<&Children>)>,
    equipment_query: Query<(&NetId, ItemQuery), With<EquippedPosition>>,
) {
    for (client_entity, client, view_key, view, mut seen, possessing, expected_state) in &mut clients {
        let Ok((character_id, character)) = owned.get(possessing.entity) else {
            continue;
        };

        let position = *character.position;
        let new_state = ExpectedCharacterState {
            body_type: **character.body_type,
            hue: **character.hue,
            flags: character.flags().bits(),
            position,
        };
        if Some(&new_state) != expected_state {
            commands.entity(client_entity).insert(new_state);
            client.send_packet(character.to_local_upsert(character_id.id));
        }

        let Some(delta_map) = delta_grid.maps.get(&position.map_id) else {
            continue;
        };

        let rect = view.0;
        let grid_min = delta_grid_cell(rect.min);
        let grid_max = delta_grid_cell(rect.max);
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
                DeltaEntry::ItemChanged { entity, parent, position, packet } => {
                    let Ok(id) = item_query.get(entity) else {
                        continue;
                    };

                    if position.map_id != view_key.map_id || !rect.contains(position.position.truncate()) {
                        if seen.remove_entity(entity) {
                            client.send_packet(DeleteEntity {
                                id: id.id,
                            });
                        }
                    } else if seen.insert_entity_if_visible(entity, parent, id.id, position.position.truncate()) {
                        client.send_packet(packet);
                    }
                }
                DeltaEntry::ItemRemoved { entity, packet, .. } => {
                    seen.remove_entity(entity);
                    client.send_packet(packet);
                }
                DeltaEntry::CharacterChanged { entity, position, update_packet, .. } => {
                    if entity == possessing.entity {
                        continue
                    }

                    if position.map_id != view_key.map_id || !rect.contains(position.position.truncate()) {
                        let Ok((id, _, _)) = character_query.get(entity) else {
                            continue;
                        };

                        if seen.remove_entity(entity) {
                            client.send_packet(DeleteEntity {
                                id: id.id,
                            });
                        }
                        continue;
                    }

                    if seen.has_seen(entity) {
                        client.send_packet(update_packet);
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
                                    slot: equipped.slot.into(),
                                    hue: **item.hue,
                                });
                            }
                        }

                        equipment.sort_by_key(|e| e.slot);
                        let packet = character.to_upsert(id.id, equipment);
                        client.send_packet(packet);
                        seen.insert_entity(entity, None, id.id, position.position.truncate());
                        seen.open_container(entity);

                        client.send_packet(character.to_status_packet(id.id));
                    }
                }
                DeltaEntry::CharacterRemoved { entity, packet, .. } => {
                    seen.remove_entity(entity);
                    client.send_packet(packet);
                }
                DeltaEntry::CharacterAnimation { entity, packet } => {
                    if seen.has_seen(entity) {
                        client.send_packet(packet);
                    }
                }
                DeltaEntry::CharacterDamaged { entity, packet } => {
                    if seen.has_seen(entity) {
                        client.send_packet(packet);
                    }
                }
                DeltaEntry::CharacterSwing { entity, packet, .. } => {
                    if seen.has_seen(entity) {
                        client.send_packet(packet);
                    }
                }
                DeltaEntry::CharacterStatusChanged { entity, packet } => {
                    if entity != possessing.entity && seen.has_seen(entity) {
                        client.send_packet(packet);
                    }
                }
                DeltaEntry::TooltipChanged { entity, packet, .. } => {
                    if seen.has_seen(entity) {
                        client.send_packet(packet);
                    }
                }
            }

            last_version = Some(delta.version);
        }
    }
}

pub fn sync_visible_entities(
    spatial_query: SpatialQuery,
    mut clients: Query<
        (&NetClient, &View, &mut LastView, &mut SeenEntities, &Possessing),
        Or<(With<Synchronizing>, With<Synchronized>)>,
    >,
    owned: Query<&MapPosition, With<OwningClient>>,
    character_query: Query<(&NetId, CharacterQuery, Option<&Children>)>,
    item_query: Query<(&NetId, ItemQuery)>,
) {
    for (client, view, mut last_view, mut seen, possessing) in &mut clients {
        let Ok(location) = owned.get(possessing.entity) else {
            continue;
        };

        let map_characters = spatial_query.characters.lookup.maps.get(&location.map_id);
        let map_items = spatial_query.dynamic_items.lookup.maps.get(&location.map_id);

        let last_rect = last_view.0;
        let rect = view_aabb(location.position.truncate(), view.range);
        let has_changed = last_view.0 != rect;
        last_view.0 = rect;

        if has_changed {
            seen.retain(|_, id, pos| {
                if rect.contains(pos) {
                    true
                } else {
                    client.send_packet(DeleteEntity { id });
                    false
                }
            });
        }

        for test_pos in rect.iter_exposed(&last_rect) {
            if let Some(characters) = map_characters {
                for entry in characters.entries_at(test_pos) {
                    let Ok((id, character, children)) = character_query.get(entry.entity) else {
                        continue;
                    };

                    seen.insert_entity(entry.entity, None, id.id, test_pos);
                    seen.open_container(entry.entity);

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

                            seen.insert_entity(*child, Some(entry.entity), child_id.id, test_pos);
                            equipment.push(CharacterEquipment {
                                id: child_id.id,
                                graphic_id: **item.graphic,
                                slot: equipped.slot.into(),
                                hue: **item.hue,
                            });
                        }
                    }

                    equipment.sort_by_key(|e| e.slot);
                    let packet = character.to_upsert(id.id, equipment);
                    client.send_packet(packet);

                    let packet = if entry.entity == possessing.entity {
                        character.to_full_status_packet(id.id)
                    } else {
                        character.to_status_packet(id.id)
                    };
                    client.send_packet(packet);
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

                    seen.insert_entity(entry.entity, None, id.id, test_pos);
                    client.send_packet(packet);
                }
            }
        }
    }
}

pub fn start_synchronizing(
    mut commands: Commands,
    maps: Res<MapInfos>,
    mut clients: Query<
        (Entity, &NetClient, Option<&ViewKey>, &mut SeenEntities, Ref<Possessing>, Has<StartedEnteringWorld>),
        Without<Synchronizing>,
    >,
    characters: Query<
        (&NetId, &MapPosition, Ref<CharacterBodyType>),
        (With<OwningClient>, With<MapPosition>),
    >,
) {
    for (entity, client, view_key, mut seen, possessing, already_in_world) in &mut clients {
        let Ok((possessed_net, map_position, body_type)) = characters.get(possessing.entity) else {
            continue;
        };

        let new_view_key = ViewKey { possessing: possessing.entity, map_id: map_position.map_id };
        if view_key == Some(&new_view_key) {
            continue;
        }

        let Some(map) = maps.maps.get(&map_position.map_id) else {
            continue;
        };

        seen.retain(|_, id, _| {
            client.send_packet(DeleteEntity { id });
            false
        });

        if !already_in_world {
            commands.entity(entity).insert(StartedEnteringWorld);
            client.send_packet(BeginEnterWorld {
                entity_id: possessed_net.id,
                body_type: **body_type,
                position: map_position.position,
                direction: map_position.direction.into(),
                map_size: map.size,
            });
        }
        client.send_packet(ExtendedCommand::ChangeMap(map_position.map_id));
        client.send_packet(ChangeSeason { season: map.season, play_sound: true });

        commands.entity(entity)
            .remove::<Synchronized>()
            .insert((
                Synchronizing,
                new_view_key,
                LastView::default(),
            ));
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
            client.send_packet(EndEnterWorld);
        }
    }
}

pub fn send_opened_containers(
    mut clients: Query<(&NetClient, &mut SeenEntities)>,
    mut events: EventReader<OnContainerOpen>,
    containers: Query<(&NetId, &Container, &RootPosition, Option<&Children>)>,
    contained_items: Query<(Entity, &NetId, ItemQuery), (With<Parent>, With<ContainedPosition>)>,
) {
    for event in events.read() {
        let Ok((client, mut seen)) = clients.get_mut(event.client_entity) else {
            continue;
        };

        let Ok((id, container, position, children)) = containers.get(event.container) else {
            continue;
        };

        seen.open_container(event.container);

        let mut contents = SmallVec::new();
        if let Some(children) = children {
            contents.reserve(children.len());

            for child in children {
                let Ok((child, child_id, item)) = contained_items.get(*child) else {
                    continue;
                };
                seen.insert_entity(child, Some(event.container), child_id.id, position.position.truncate());
                contents.push(item.to_upsert_contained(child_id.id, id.id).unwrap());
            }
        }

        client.send_packet(OpenContainer {
            id: id.id,
            gump_id: container.gump_id,
        });
        client.send_packet(UpsertContainerContents {
            contents,
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<View>()
        .register_type::<LastView>()
        .register_type::<OwningClient>()
        .register_type::<StartedEnteringWorld>()
        .register_type::<EnteredWorld>()
        .register_type::<ExpectedCharacterState>()
        .add_systems(Last, (
            start_synchronizing,
        ).in_set(ServerSet::SendFirst))
        .add_systems(Last, (
            (
                send_deltas,
                sync_visible_entities,
            ).chain(),
        ).in_set(ServerSet::SendEntities))
        .add_systems(Last, (
            send_opened_containers,
            finish_synchronizing,
        ).in_set(ServerSet::Send));
}
