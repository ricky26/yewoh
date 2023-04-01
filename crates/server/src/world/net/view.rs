use std::collections::{HashMap, HashSet, VecDeque};

use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::{Changed, Or, With, Without};
use bevy_ecs::removal_detection::RemovedComponents;
use bevy_ecs::system::{Commands, Local, Query, Res, Resource};
use bevy_ecs::world::{Mut, Ref};
use bevy_reflect::Reflect;
use bitflags::bitflags;
use glam::UVec2;

use yewoh::{EntityKind, Notoriety};
use yewoh::protocol::{CharacterEquipment, DeleteEntity, EntityFlags, EntityTooltipVersion, UpdateCharacter, UpsertContainerContents, UpsertEntityCharacter, UpsertEntityContained, UpsertEntityEquipped, UpsertEntityWorld, UpsertLocalPlayer};
use yewoh::protocol::{BeginEnterWorld, ChangeSeason, EndEnterWorld, ExtendedCommand};

use crate::world::entity::{Character, Container, EquippedBy, Flags, Graphic, Location, Notorious, ParentContainer, Quantity, Stats, Tooltip};
use crate::world::net::{NetClient, NetEntity, NetEntityLookup, NetOwner};
use crate::world::net::connection::Possessing;
use crate::world::spatial::{EntityPositions, NetClientPositions, view_aabb};

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

#[derive(Debug, Clone, Component)]
pub struct View {
    pub range: i32,
}

#[derive(Default, Debug, Clone, Component)]
pub struct VisibleContainers {
    pub containers: HashSet<Entity>,
}

#[derive(Debug, Clone, Component)]
pub struct PartiallyVisible {
    pub visible_to: HashSet<Entity>,
}

impl PartiallyVisible {
    pub fn is_visible_to(&self, entity: Entity) -> bool {
        self.visible_to.contains(&entity)
    }
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct Synchronizing;

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct Synchronized;

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct EnteredWorld;

bitflags! {
    #[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
    pub struct CharacterDirtyFlags : u8 {
        const UPSERT = 1 << 0;
        const REMOVE = 1 << 1;
        const UPDATE = 1 << 2;
        const STATS = 1 << 3;
    }

    #[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
    pub struct ItemDirtyFlags : u8 {
        const UPSERT = 1 << 0;
        const REMOVE = 1 << 1;
        const CONTENTS = 1 << 2;
        const TOOLTIP = 1 << 3;
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CharacterState {
    pub dirty_flags: CharacterDirtyFlags,
    pub position: Location,
    pub body_type: u16,
    pub hue: u16,
    pub notoriety: Notoriety,
    pub stats: Stats,
    pub flags: EntityFlags,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorldItemState {
    pub position: Location,
    pub flags: EntityFlags,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ItemPositionState {
    World(WorldItemState),
    Equipped(EquippedBy),
    Contained(ParentContainer),
}

#[derive(Debug, Clone)]
pub struct ItemState {
    pub dirty_flags: ItemDirtyFlags,
    pub graphic: Graphic,
    pub position: ItemPositionState,
    pub quantity: u16,
    pub tooltip: Tooltip,
    pub tooltip_version: u32,
}

impl ItemState {
    pub fn parent(&self) -> Option<Entity> {
        match &self.position {
            ItemPositionState::World(_) => None,
            ItemPositionState::Equipped(by) =>
                Some(by.parent),
            ItemPositionState::Contained(parent) =>
                Some(parent.parent),
        }
    }
}

#[derive(Debug, Clone)]
pub enum GhostState {
    Character(CharacterState),
    Item(ItemState),
}

impl GhostState {
    pub fn is_dirty(&self) -> bool {
        match self {
            GhostState::Character(character) =>
                character.dirty_flags != CharacterDirtyFlags::empty(),
            GhostState::Item(item) =>
                item.dirty_flags != ItemDirtyFlags::empty(),
        }
    }

    pub fn parent(&self) -> Option<Entity> {
        match self {
            GhostState::Character(_) => None,
            GhostState::Item(item) => item.parent(),
        }
    }
}

#[derive(Default, Debug, Component)]
pub struct ViewState {
    ghosts: HashMap<Entity, GhostState>,
    children: HashMap<Entity, HashSet<Entity>>,
    map_id: u8,
    possessed: Option<Entity>,
    dirty: bool,
}

impl ViewState {
    pub fn new() -> ViewState {
        Self {
            ghosts: Default::default(),
            children: Default::default(),
            map_id: 0xff,
            possessed: None,
            dirty: false,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn flush(&mut self) {
        self.ghosts.clear();
        self.children.clear();
        self.dirty = false;
    }

    pub fn has_ghost(&self, entity: Entity) -> bool {
        self.ghosts.contains_key(&entity)
    }

    pub fn upsert_ghost(&mut self, entity: Entity, mut state: GhostState) {
        let previous = self.ghosts.remove(&entity);
        let previous_parent = previous.as_ref().and_then(|s| s.parent());

        match (previous, &mut state) {
            // Character update
            (Some(GhostState::Character(old)), GhostState::Character(new)) => {
                // Preserve stats, this is set otherwise.
                new.stats = old.stats;

                if old.body_type != new.body_type
                    || old.hue != new.hue
                    || old.flags != new.flags
                    || old.notoriety != new.notoriety
                    || old.position != new.position {
                    new.dirty_flags = CharacterDirtyFlags::UPDATE;
                }
            }
            // New character
            (_, GhostState::Character(new)) => {
                new.dirty_flags = CharacterDirtyFlags::UPSERT | CharacterDirtyFlags::STATS;
            }
            // Item Update
            (Some(GhostState::Item(old)), GhostState::Item(new)) => {
                // Preserve tooltip for now, as this is set by another method.
                new.tooltip = old.tooltip;
                new.tooltip_version = old.tooltip_version;
                new.dirty_flags = old.dirty_flags;

                if new.quantity != old.quantity
                    || new.graphic != old.graphic
                    || new.position != old.position {
                    new.dirty_flags = ItemDirtyFlags::UPSERT;
                }
            }
            // New Item
            (_, GhostState::Item(new)) => {
                new.dirty_flags = ItemDirtyFlags::UPSERT | ItemDirtyFlags::CONTENTS | ItemDirtyFlags::TOOLTIP;
            }
        }

        if state.is_dirty() {
            self.dirty = true;
        }

        let parent = state.parent();
        if previous_parent != parent {
            if let Some(parent) = previous_parent {
                self.remove_child(parent, entity);
            }

            if let Some(parent) = parent {
                self.add_child(parent, entity);
            }
        }

        self.ghosts.insert(entity, state);
    }

    pub fn remove_ghost(&mut self, entity: Entity) {
        match self.ghosts.get_mut(&entity) {
            Some(GhostState::Character(character)) =>
                character.dirty_flags |= CharacterDirtyFlags::REMOVE,
            Some(GhostState::Item(item)) =>
                item.dirty_flags |= ItemDirtyFlags::REMOVE,
            None => return,
        }
        self.dirty = true;
    }

    fn add_child(&mut self, parent: Entity, child: Entity) {
        self.children.entry(parent).or_default().insert(child);
    }

    fn remove_child(&mut self, parent: Entity, child: Entity) {
        if let Some(children) = self.children.get_mut(&parent) {
            children.remove(&child);

            if children.is_empty() {
                self.children.remove(&parent);
            }
        }
    }

    fn remove_out_of_range(&mut self, position: Location, range: i32, containers: &HashSet<Entity>) {
        let position_in_range = |p: Location| position.in_range(&p, range);
        let mut to_remove = Vec::new();

        for (entity, ghost) in &self.ghosts {
            let in_range = match ghost {
                GhostState::Character(character) =>
                    position_in_range(character.position),
                GhostState::Item(item) => {
                    match &item.position {
                        ItemPositionState::World(world) =>
                            position_in_range(world.position),
                        ItemPositionState::Equipped(by) => {
                            if let Some(GhostState::Character(character)) = self.ghosts.get(&by.parent) {
                                position_in_range(character.position)
                            } else {
                                false
                            }
                        }
                        ItemPositionState::Contained(container) =>
                            containers.contains(&container.parent),
                    }
                }
            };

            if in_range {
                continue;
            }

            to_remove.push((*entity, ghost.parent()));
        }

        for (entity, parent) in to_remove.into_iter() {
            if let Some(parent) = parent {
                self.remove_child(parent, entity);
            }
            self.ghosts.remove(&entity);
        }
    }

    pub fn set_tooltip(&mut self, entity: Entity, tooltip: Tooltip) {
        match self.ghosts.get_mut(&entity) {
            Some(GhostState::Item(item)) => {
                item.tooltip = tooltip;
                item.tooltip_version += 1;
                item.dirty_flags |= ItemDirtyFlags::TOOLTIP;
                self.dirty = true;
            }
            _ => {}
        }
    }

    pub fn set_stats(&mut self, entity: Entity, stats: Stats) {
        match self.ghosts.get_mut(&entity) {
            Some(GhostState::Character(character)) => {
                character.stats = stats;
                character.dirty_flags |= CharacterDirtyFlags::STATS;
                self.dirty = true;
            }
            _ => {}
        }
    }
}

pub fn is_visible_to(viewer: Entity, visibility: &Option<&PartiallyVisible>) -> bool {
    visibility.as_ref().map_or(true, |v| v.is_visible_to(viewer))
}

pub fn update_tooltip(
    view_state: &mut Mut<ViewState>, entity: Entity, tooltip: &Tooltip,
) {
    match view_state.ghosts.get(&entity) {
        Some(GhostState::Item(item)) => {
            if &item.tooltip == tooltip {
                return;
            }
        }
        _ => return,
    }

    view_state.set_tooltip(entity, tooltip.clone());
}

pub fn update_tooltips(
    mut clients: Query<&mut ViewState>,
    tooltips: Query<(Entity, Ref<Tooltip>), Changed<Tooltip>>,
) {
    for (entity, tooltip) in tooltips.iter() {
        for mut view_state in clients.iter_mut() {
            update_tooltip(&mut view_state, entity, &tooltip);
        }
    }
}

pub fn update_stats(
    mut clients: Query<&mut ViewState>,
    stats: Query<(Entity, Ref<Stats>), Changed<Stats>>,
) {
    for (entity, stats) in stats.iter() {
        for mut view_state in clients.iter_mut() {
            match view_state.ghosts.get(&entity) {
                Some(GhostState::Character(character)) => {
                    if &character.stats == stats.as_ref() {
                        return;
                    }
                }
                _ => return,
            }

            view_state.set_stats(entity, stats.clone());
        }
    }
}

pub fn send_ghost_updates(
    entity_lookup: Res<NetEntityLookup>,
    mut clients: Query<
        (&NetClient, &View, &mut ViewState, &Possessing, &VisibleContainers),
        Changed<ViewState>,
    >,
    positioned: Query<&Location, With<NetOwner>>,
    mut to_visit: Local<VecDeque<Entity>>,
    mut to_remove: Local<Vec<(Entity, Option<Entity>)>>,
) {
    for (client, view, mut view_state, possessing, containers) in clients.iter_mut() {
        let position = match positioned.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        if !view_state.dirty {
            continue;
        }

        let mut view_state = view_state.as_mut();
        view_state.dirty = false;
        view_state.remove_out_of_range(position.clone(), view.range, &containers.containers);

        let mut new_parents = HashSet::new();

        to_remove.clear();
        to_visit.clear();
        to_visit.extend(view_state.ghosts.iter()
            .filter(|(_, ghost)| ghost.parent().is_none())
            .map(|(entity, _)| *entity));

        while let Some(entity) = to_visit.pop_back() {
            let id = match entity_lookup.ecs_to_net(entity) {
                Some(x) => x,
                None => continue,
            };

            let mut equipment = Vec::new();
            let mut contents = Vec::new();

            match view_state.ghosts.get(&entity) {
                Some(GhostState::Character(character)) => {
                    if character.dirty_flags.contains(CharacterDirtyFlags::UPSERT) {
                        if let Some(children) = view_state.children.get(&entity) {
                            for child_entity in children.iter().copied() {
                                let item = match view_state.ghosts.get(&child_entity) {
                                    Some(GhostState::Item(x)) => x,
                                    _ => continue,
                                };
                                let by = match &item.position {
                                    ItemPositionState::Equipped(by) => by,
                                    _ => continue,
                                };
                                let child_id = match entity_lookup.ecs_to_net(child_entity) {
                                    Some(x) => x,
                                    None => continue,
                                };
                                equipment.push(CharacterEquipment {
                                    id: child_id,
                                    slot: by.slot,
                                    graphic_id: item.graphic.id,
                                    hue: item.graphic.hue,
                                });
                            }
                        }
                    }
                }
                Some(GhostState::Item(item)) => {
                    if containers.containers.contains(&entity) && item.dirty_flags.contains(ItemDirtyFlags::CONTENTS) {
                        if let Some(children) = view_state.children.get(&entity) {
                            for child_entity in children.iter().copied() {
                                let item = match view_state.ghosts.get(&child_entity) {
                                    Some(GhostState::Item(x)) => x,
                                    _ => continue,
                                };
                                let container = match &item.position {
                                    ItemPositionState::Contained(x) => x,
                                    _ => continue,
                                };
                                let child_id = match entity_lookup.ecs_to_net(child_entity) {
                                    Some(x) => x,
                                    None => continue,
                                };
                                contents.push(UpsertEntityContained {
                                    id: child_id,
                                    graphic_id: item.graphic.id,
                                    graphic_inc: 0,
                                    quantity: item.quantity,
                                    position: container.position,
                                    grid_index: container.grid_index,
                                    parent_id: id,
                                    hue: item.graphic.hue,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }

            let state = match view_state.ghosts.get_mut(&entity) {
                Some(x) => x,
                None => continue,
            };
            match state {
                GhostState::Character(character) => {
                    let dirty_flags = std::mem::replace(&mut character.dirty_flags, CharacterDirtyFlags::empty());

                    if dirty_flags.contains(CharacterDirtyFlags::REMOVE) {
                        to_remove.push((entity, None));
                        client.send_packet(DeleteEntity {
                            id,
                        }.into());
                    } else {
                        if dirty_flags.contains(CharacterDirtyFlags::UPSERT) {
                            new_parents.insert(entity);
                            client.send_packet(UpsertEntityCharacter {
                                id,
                                body_type: character.body_type,
                                position: character.position.position,
                                direction: character.position.direction,
                                hue: character.hue,
                                flags: character.flags,
                                notoriety: character.notoriety,
                                equipment,
                            }.into());
                        } else if dirty_flags.contains(CharacterDirtyFlags::UPDATE) {
                            client.send_packet(UpdateCharacter {
                                id,
                                body_type: character.body_type,
                                position: character.position.position,
                                direction: character.position.direction,
                                hue: character.hue,
                                flags: character.flags,
                                notoriety: character.notoriety,
                            }.into());
                        }

                        if dirty_flags.contains(CharacterDirtyFlags::STATS) {
                            client.send_packet(character.stats.upsert(id, Some(entity) == view_state.possessed).into());
                        }

                        if Some(entity) == view_state.possessed && dirty_flags.contains(CharacterDirtyFlags::UPSERT) {
                            client.send_packet(UpsertLocalPlayer {
                                id,
                                body_type: character.body_type,
                                server_id: 0,
                                hue: character.hue,
                                flags: character.flags,
                                position: character.position.position,
                                direction: character.position.direction,
                            }.into());
                        }
                    }
                }
                GhostState::Item(item) => {
                    let dirty_flags = std::mem::replace(&mut item.dirty_flags, ItemDirtyFlags::empty());

                    if dirty_flags.contains(ItemDirtyFlags::REMOVE) {
                        client.send_packet(DeleteEntity {
                            id,
                        }.into());
                        to_remove.push((entity, item.parent()));
                    } else {
                        match &item.position {
                            ItemPositionState::World(world) => {
                                if !dirty_flags.is_empty() {
                                    client.send_packet(UpsertEntityWorld {
                                        id,
                                        kind: EntityKind::Item,
                                        graphic_id: item.graphic.id,
                                        graphic_inc: 0,
                                        direction: world.position.direction,
                                        quantity: item.quantity,
                                        position: world.position.position,
                                        hue: item.graphic.hue,
                                        flags: world.flags,
                                    }.into());
                                }
                            }
                            ItemPositionState::Contained(container) => {
                                if !new_parents.contains(&container.parent) && !dirty_flags.is_empty() {
                                    if let Some(parent_id) = entity_lookup.ecs_to_net(container.parent) {
                                        client.send_packet(UpsertEntityContained {
                                            id,
                                            graphic_id: item.graphic.id,
                                            graphic_inc: 0,
                                            quantity: item.quantity,
                                            position: container.position,
                                            grid_index: container.grid_index,
                                            parent_id,
                                            hue: item.graphic.hue,
                                        }.into());
                                    }
                                }
                            }
                            ItemPositionState::Equipped(by) => {
                                if !new_parents.contains(&by.parent) && !dirty_flags.is_empty() {
                                    if let Some(parent_id) = entity_lookup.ecs_to_net(by.parent) {
                                        client.send_packet(UpsertEntityEquipped {
                                            id,
                                            parent_id,
                                            slot: by.slot,
                                            graphic_id: item.graphic.id,
                                            hue: item.graphic.hue,
                                        }.into());
                                    }
                                }
                            }
                        }

                        if containers.containers.contains(&entity) && dirty_flags.contains(ItemDirtyFlags::CONTENTS) && !contents.is_empty() {
                            new_parents.insert(entity);
                            client.send_packet(UpsertContainerContents {
                                contents,
                            }.into());
                        }

                        if dirty_flags.contains(ItemDirtyFlags::TOOLTIP) && item.tooltip_version > 0 {
                            client.send_packet(EntityTooltipVersion {
                                id,
                                revision: item.tooltip_version,
                            }.into());
                        }
                    }
                }
            }

            if let Some(kids) = view_state.children.get(&entity) {
                to_visit.extend(kids.iter().cloned());
            }
        }

        for (entity, parent) in to_remove.drain(..) {
            if let Some(parent) = parent {
                view_state.remove_child(parent, entity);
            }

            view_state.ghosts.remove(&entity);
        }
    }
}

pub fn sync_nearby(
    entity_positions: Res<EntityPositions>,
    mut clients: Query<
        (&View, &mut ViewState, &Possessing, &VisibleContainers),
        (With<Synchronizing>, Without<Synchronized>),
    >,
    world_items: Query<(
        &Flags, &Graphic, &Location, Option<&Quantity>, Option<&Tooltip>,
        Option<&PartiallyVisible>
    )>,
    characters: Query<(
        &Flags, &Character, &Location, &Notorious, &Stats, Option<&PartiallyVisible>,
    )>,
    equipped_items: Query<(
        &Graphic, &EquippedBy, Option<&Tooltip>, Option<&PartiallyVisible>
    )>,
    containers: Query<&Container>,
    child_items: Query<(
        &Graphic, &ParentContainer, Option<&Quantity>, Option<&Tooltip>,
        Option<&PartiallyVisible>
    )>,
) {
    for (view, mut view_state, possessing, visible_containers) in clients.iter_mut() {
        let (_, _, position, _, _, _) = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };
        let (min, max) = view_aabb(position.position.truncate(), view.range);
        for (entity, _) in entity_positions.tree.iter_aabb(position.map_id, min, max) {
            if let Ok((flags, graphic, position, quantity, tooltip, visibility)) = world_items.get(entity) {
                if is_visible_to(possessing.entity, &visibility) {
                    view_state.upsert_ghost(entity, GhostState::Item(ItemState {
                        dirty_flags: ItemDirtyFlags::empty(),
                        graphic: *graphic,
                        position: ItemPositionState::World(WorldItemState {
                            position: *position,
                            flags: flags.flags,
                        }),
                        quantity: quantity.map_or(1, |q| q.quantity),
                        tooltip: Default::default(),
                        tooltip_version: 0,
                    }));

                    if let Some(tooltip) = tooltip {
                        update_tooltip(&mut view_state, entity, tooltip);
                    }
                }
            } else if let Ok((flags, character, position, notorious, stats, visibility)) = characters.get(entity) {
                if is_visible_to(possessing.entity, &visibility) {
                    view_state.upsert_ghost(entity, GhostState::Character(CharacterState {
                        dirty_flags: CharacterDirtyFlags::empty(),
                        position: *position,
                        body_type: character.body_type,
                        hue: character.hue,
                        notoriety: notorious.0,
                        stats: stats.clone(),
                        flags: flags.flags,
                    }));

                    for equipped in &character.equipment {
                        if let Ok((graphic, parent, tooltip, visibility)) = equipped_items.get(equipped.equipment) {
                            if is_visible_to(possessing.entity, &visibility) {
                                view_state.upsert_ghost(equipped.equipment, GhostState::Item(ItemState {
                                    dirty_flags: ItemDirtyFlags::empty(),
                                    graphic: *graphic,
                                    position: ItemPositionState::Equipped(parent.clone()),
                                    quantity: 1,
                                    tooltip: Default::default(),
                                    tooltip_version: 0,
                                }));

                                if let Some(tooltip) = tooltip {
                                    update_tooltip(&mut view_state, equipped.equipment, tooltip);
                                }
                            }
                        }
                    }
                }
            }
        }

        for entity in visible_containers.containers.iter().copied() {
            let container = match containers.get(entity) {
                Ok(x) => x,
                _ => continue,
            };

            for child_entity in container.items.iter().copied() {
                if let Ok((graphic, parent, quantity, tooltip, visibility)) = child_items.get(child_entity) {
                    if is_visible_to(possessing.entity, &visibility) {
                        view_state.upsert_ghost(child_entity, GhostState::Item(ItemState {
                            dirty_flags: ItemDirtyFlags::empty(),
                            graphic: *graphic,
                            position: ItemPositionState::Contained(parent.clone()),
                            quantity: quantity.map_or(1, |q| q.quantity),
                            tooltip: Default::default(),
                            tooltip_version: 0,
                        }));

                        if let Some(tooltip) = tooltip {
                            update_tooltip(&mut view_state, child_entity, tooltip);
                        }
                    }
                }
            }
        }
    }
}

pub fn update_nearby_moving(
    entity_positions: Res<EntityPositions>,
    mut clients: Query<(&View, &mut ViewState, &Possessing), With<Synchronized>>,
    moved_characters: Query<(&NetOwner, &Location), Changed<Location>>,
    world_items: Query<(
        &Flags, &Graphic, &Location, Option<&Quantity>, Option<&Tooltip>,
        Option<&PartiallyVisible>
    )>,
    characters: Query<(
        &Flags, &Character, &Location, &Notorious, &Stats, Option<&PartiallyVisible>,
    )>,
    equipped_items: Query<(
        &Graphic, &EquippedBy, Option<&Tooltip>, Option<&PartiallyVisible>
    )>,
) {
    for (owner, position) in moved_characters.iter() {
        let (view, mut view_state, possessing) = match clients.get_mut(owner.client_entity) {
            Ok(x) => x,
            _ => continue,
        };
        let (min, max) = view_aabb(position.position.truncate(), view.range);
        for (entity, _) in entity_positions.tree.iter_aabb(position.map_id, min, max) {
            if let Ok((flags, graphic, position, quantity, tooltip, visibility)) = world_items.get(entity) {
                if is_visible_to(possessing.entity, &visibility) {
                    view_state.upsert_ghost(entity, GhostState::Item(ItemState {
                        dirty_flags: ItemDirtyFlags::empty(),
                        graphic: *graphic,
                        position: ItemPositionState::World(WorldItemState {
                            position: *position,
                            flags: flags.flags,
                        }),
                        quantity: quantity.map_or(1, |q| q.quantity),
                        tooltip: Default::default(),
                        tooltip_version: 0,
                    }));

                    if let Some(tooltip) = tooltip {
                        update_tooltip(&mut view_state, entity, tooltip);
                    }
                }
            } else if let Ok((flags, character, position, notorious, stats, visibility)) = characters.get(entity) {
                if is_visible_to(possessing.entity, &visibility) {
                    view_state.upsert_ghost(entity, GhostState::Character(CharacterState {
                        dirty_flags: CharacterDirtyFlags::empty(),
                        position: *position,
                        body_type: character.body_type,
                        hue: character.hue,
                        notoriety: notorious.0,
                        stats: stats.clone(),
                        flags: flags.flags,
                    }));

                    for equipped in &character.equipment {
                        if let Ok((graphic, parent, tooltip, visibility)) = equipped_items.get(equipped.equipment) {
                            if is_visible_to(possessing.entity, &visibility) {
                                view_state.upsert_ghost(equipped.equipment, GhostState::Item(ItemState {
                                    dirty_flags: ItemDirtyFlags::empty(),
                                    graphic: *graphic,
                                    position: ItemPositionState::Equipped(parent.clone()),
                                    quantity: 1,
                                    tooltip: Default::default(),
                                    tooltip_version: 0,
                                }));

                                if let Some(tooltip) = tooltip {
                                    update_tooltip(&mut view_state, equipped.equipment, tooltip);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn update_nearby(
    client_positions: Res<NetClientPositions>,
    mut clients: Query<(&mut ViewState, &Possessing), (With<Synchronized>, Without<Synchronizing>)>,
    world_items: Query<
        (Entity, &Flags, &Graphic, &Location, Option<&Quantity>, Option<&PartiallyVisible>),
        Or<(Changed<Flags>, Changed<Graphic>, Changed<Location>, Changed<Quantity>, Changed<PartiallyVisible>)>,
    >,
    characters: Query<
        (Entity, &Flags, &Character, &Location, &Notorious, Option<&PartiallyVisible>),
        Or<(Changed<Flags>, Changed<Character>, Changed<Location>, Changed<Notorious>, Changed<PartiallyVisible>)>,
    >,
    equipped_items: Query<(
        &Graphic, &EquippedBy, Option<&Tooltip>, Option<&PartiallyVisible>
    )>,
    mut removed_entities: RemovedComponents<NetEntity>,
) {
    for (entity, flags, graphic, position, quantity, visibility) in world_items.iter() {
        for (client_entity, _) in client_positions.tree.iter_at_point(position.map_id, position.position.truncate()) {
            let (mut view_state, possessing) = match clients.get_mut(client_entity) {
                Ok(x) => x,
                _ => continue,
            };

            if is_visible_to(possessing.entity, &visibility) {
                view_state.upsert_ghost(entity, GhostState::Item(ItemState {
                    dirty_flags: ItemDirtyFlags::empty(),
                    graphic: *graphic,
                    position: ItemPositionState::World(WorldItemState {
                        position: *position,
                        flags: flags.flags,
                    }),
                    quantity: quantity.map_or(1, |q| q.quantity),
                    tooltip: Default::default(),
                    tooltip_version: 0,
                }));
            } else if view_state.has_ghost(entity) {
                view_state.remove_ghost(entity);
            }
        }
    }

    for (entity, flags, character, position, notorious, visibility) in characters.iter() {
        for (client_entity, _) in client_positions.tree.iter_at_point(position.map_id, position.position.truncate()) {
            let (mut view_state, possessing) = match clients.get_mut(client_entity) {
                Ok(x) => x,
                _ => continue,
            };

            if is_visible_to(possessing.entity, &visibility) {
                view_state.upsert_ghost(entity, GhostState::Character(CharacterState {
                    dirty_flags: CharacterDirtyFlags::empty(),
                    position: *position,
                    body_type: character.body_type,
                    hue: character.hue,
                    notoriety: notorious.0,
                    stats: Default::default(),
                    flags: flags.flags,
                }));

                for equipped in &character.equipment {
                    if let Ok((graphic, parent, tooltip, visibility)) = equipped_items.get(equipped.equipment) {
                        if is_visible_to(possessing.entity, &visibility) {
                            view_state.upsert_ghost(equipped.equipment, GhostState::Item(ItemState {
                                dirty_flags: ItemDirtyFlags::empty(),
                                graphic: *graphic,
                                position: ItemPositionState::Equipped(parent.clone()),
                                quantity: 1,
                                tooltip: Default::default(),
                                tooltip_version: 0,
                            }));

                            if let Some(tooltip) = tooltip {
                                update_tooltip(&mut view_state, equipped.equipment, tooltip);
                            }
                        } else if view_state.has_ghost(equipped.equipment) {
                            view_state.remove_ghost(equipped.equipment);
                        }
                    }
                }
            } else if view_state.has_ghost(entity) {
                view_state.remove_ghost(entity);
            }
        }
    }

    for entity in removed_entities.iter() {
        for (mut view_state, _) in clients.iter_mut() {
            if view_state.has_ghost(entity) {
                view_state.remove_ghost(entity);
            }
        }
    }
}

pub fn update_items_in_containers(
    mut clients: Query<(&mut ViewState, &Possessing, &VisibleContainers), (With<Synchronized>, Without<Synchronizing>)>,
    items: Query<
        (Entity, &Graphic, &ParentContainer, Option<&Quantity>, Option<&PartiallyVisible>),
        Or<(Changed<Graphic>, Changed<ParentContainer>, Changed<Quantity>, Changed<PartiallyVisible>)>,
    >,
) {
    for (entity, graphic, parent, quantity, visibility) in items.iter() {
        // TODO: we can recurse up to find the position of the root container first.
        for (mut view_state, possessing, visible_containers) in clients.iter_mut() {
            if !visible_containers.containers.contains(&parent.parent) {
                continue;
            }

            if is_visible_to(possessing.entity, &visibility) {
                view_state.upsert_ghost(entity, GhostState::Item(ItemState {
                    dirty_flags: ItemDirtyFlags::empty(),
                    graphic: *graphic,
                    position: ItemPositionState::Contained(parent.clone()),
                    quantity: quantity.map_or(1, |q| q.quantity),
                    tooltip: Default::default(),
                    tooltip_version: 0,
                }));
            } else if view_state.has_ghost(entity) {
                view_state.remove_ghost(entity);
            }
        }
    }
}

pub fn update_equipped_items(
    client_positions: Res<NetClientPositions>,
    mut clients: Query<(&mut ViewState, &Possessing), (With<Synchronized>, Without<Synchronizing>)>,
    items: Query<
        (Entity, &Graphic, &EquippedBy, Option<&PartiallyVisible>),
        Or<(Changed<Graphic>, Changed<EquippedBy>, Changed<PartiallyVisible>)>,
    >,
    characters: Query<&Location, With<Character>>,
) {
    for (entity, graphic, parent,visibility) in items.iter() {
        let position = match characters.get(parent.parent) {
            Ok(x) => x,
            _ => continue,
        };

        for (client_entity, _) in client_positions.tree.iter_at_point(position.map_id, position.position.truncate()) {
            let (mut view_state, possessing) = match clients.get_mut(client_entity) {
                Ok(x) => x,
                _ => continue,
            };

            if is_visible_to(possessing.entity, &visibility) {
                view_state.upsert_ghost(entity, GhostState::Item(ItemState {
                    dirty_flags: ItemDirtyFlags::empty(),
                    graphic: *graphic,
                    position: ItemPositionState::Equipped(parent.clone()),
                    quantity: 1,
                    tooltip: Default::default(),
                    tooltip_version: 0,
                }));
            } else if view_state.has_ghost(entity) {
                view_state.remove_ghost(entity);
            }
        }
    }
}

pub fn start_synchronizing(
    clients: Query<(Entity, &ViewState, Ref<Possessing>), Without<Synchronizing>>,
    characters: Query<&Location, With<NetOwner>>,
    mut commands: Commands,
) {
    for (entity, view_state, possessing) in clients.iter() {
        let map_position = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        if !possessing.is_changed() && view_state.map_id == map_position.map_id {
            continue;
        }

        commands.entity(entity).remove::<Synchronized>().insert(Synchronizing);
    }
}

pub fn send_change_map(
    mut clients: Query<(&NetClient, &mut ViewState, Ref<Possessing>), With<Synchronizing>>,
    characters: Query<(&NetEntity, &Location, Ref<Character>)>,
    maps: Res<MapInfos>,
) {
    for (client, mut view_state, possessing) in clients.iter_mut() {
        let (possessed_net, map_position, character) = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        let map = match maps.maps.get(&map_position.map_id) {
            Some(v) => v,
            None => continue,
        };

        if possessing.is_changed() {
            // should BeginEnterWorld be here?
        } else if view_state.map_id == map_position.map_id {
            continue;
        }

        view_state.map_id = map_position.map_id;
        view_state.possessed = Some(possessing.entity);
        view_state.flush();

        client.send_packet(BeginEnterWorld {
            entity_id: possessed_net.id,
            body_type: character.body_type,
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
