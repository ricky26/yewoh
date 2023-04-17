use std::collections::{HashMap, HashSet, VecDeque};

use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EventReader;
use bevy_ecs::query::{With, Without};
use bevy_ecs::system::{Commands, Query, Res, Resource, SystemParam};
use bevy_ecs::world::{Mut, Ref};
use bevy_reflect::Reflect;
use bitflags::bitflags;
use glam::UVec2;

use yewoh::{EntityKind, Notoriety};
use yewoh::protocol::{CharacterEquipment, DeleteEntity, EntityFlags, EntityTooltipVersion, EquipmentSlot, OpenContainer, UpdateCharacter, UpsertContainerContents, UpsertEntityCharacter, UpsertEntityContained, UpsertEntityEquipped, UpsertEntityWorld, UpsertLocalPlayer};
use yewoh::protocol::{BeginEnterWorld, ChangeSeason, EndEnterWorld, ExtendedCommand};

use crate::world::entity::{Character, Container, EquippedBy, Flags, Graphic, Location, Notorious, ParentContainer, Quantity, Stats, Tooltip};
use crate::world::net::{NetClient, NetEntity, NetEntityLookup, NetOwner};
use crate::world::net::connection::Possessing;
use crate::world::spatial::{EntityPositions, view_aabb};

#[derive(Debug, Clone)]
pub struct ContainerOpenedEvent {
    pub client_entity: Entity,
    pub container: Entity,
}

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

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct Synchronizing;

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct Synchronized;

#[derive(Debug, Clone, Copy, Component, Reflect)]
pub struct EnteredWorld;

bitflags! {
    #[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
    struct CharacterDirtyFlags : u8 {
        const UPSERT = 1 << 0;
        const UPDATE = 1 << 1;
        const STATS = 1 << 2;
    }

    #[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
    struct ItemDirtyFlags : u8 {
        const UPSERT = 1 << 0;
        const TOOLTIP = 1 << 1;
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CharacterState {
    dirty_flags: CharacterDirtyFlags,
    location: Location,
    body_type: u16,
    hue: u16,
    notoriety: Notoriety,
    stats: Stats,
    flags: EntityFlags,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct WorldItemState {
    location: Location,
    flags: EntityFlags,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ItemPositionState {
    World(WorldItemState),
    Equipped(EquippedBy),
    Contained(ParentContainer),
}

#[derive(Debug, Clone)]
struct ItemState {
    dirty_flags: ItemDirtyFlags,
    graphic: Graphic,
    position: ItemPositionState,
    quantity: u16,
    container_gump: Option<u16>,
    tooltip: Tooltip,
    tooltip_version: u32,
}

impl ItemState {
    fn parent(&self) -> Option<Entity> {
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
enum GhostState {
    Character(CharacterState),
    Item(ItemState),
}

impl GhostState {
    fn is_dirty(&self) -> bool {
        match self {
            GhostState::Character(character) =>
                character.dirty_flags != CharacterDirtyFlags::empty(),
            GhostState::Item(item) =>
                item.dirty_flags != ItemDirtyFlags::empty(),
        }
    }

    fn parent(&self) -> Option<Entity> {
        match self {
            GhostState::Character(_) => None,
            GhostState::Item(item) => item.parent(),
        }
    }
}

#[derive(Debug, Component)]
pub struct ViewState {
    ghosts: HashMap<Entity, GhostState>,
    children: HashMap<Entity, HashSet<Entity>>,
    to_remove: HashSet<Entity>,
    to_visit: VecDeque<Entity>,
    map_id: u8,
    possessed: Option<Entity>,
    dirty: bool,
}

impl ViewState {
    pub fn new() -> ViewState {
        Self {
            ghosts: Default::default(),
            children: Default::default(),
            to_remove: Default::default(),
            to_visit: Default::default(),
            map_id: 0xff,
            possessed: None,
            dirty: false,
        }
    }

    pub fn map_id(&self) -> u8 {
        self.map_id
    }

    fn flush(&mut self) {
        self.ghosts.clear();
        self.children.clear();
        self.to_remove.clear();
        self.dirty = false;
    }

    fn mark_unseen(&mut self) {
        self.to_remove.extend(self.ghosts.keys().copied());
    }

    fn upsert_ghost(&mut self, entity: Entity, mut state: GhostState) {
        let previous = self.ghosts.remove(&entity);
        let previous_parent = previous.as_ref().and_then(|s| s.parent());

        self.to_remove.remove(&entity);

        match (previous, &mut state) {
            // Character update
            (Some(GhostState::Character(old)), GhostState::Character(new)) => {
                if old.body_type != new.body_type
                    || old.hue != new.hue
                    || old.flags != new.flags
                    || old.notoriety != new.notoriety
                    || old.location != new.location {
                    new.dirty_flags |= CharacterDirtyFlags::UPDATE;
                }

                if old.stats != new.stats {
                    new.dirty_flags |= CharacterDirtyFlags::STATS;
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
                new.dirty_flags = ItemDirtyFlags::UPSERT | ItemDirtyFlags::TOOLTIP;
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

    fn for_each_removal(&mut self, mut f: impl FnMut(Entity, GhostState)) {
        for entity in self.to_remove.drain() {
            let state = match self.ghosts.remove(&entity) {
                Some(v) => v,
                _ => continue,
            };

            if let Some(parent) = state.parent() {
                remove_child(&mut self.children, parent, entity);
            }

            (f)(entity, state);
        }
    }

    fn for_each_ghost(&mut self, mut f: impl FnMut(&Self, Entity, &mut GhostState)) {
        self.to_visit.clear();
        self.to_visit.extend(self.ghosts.iter()
            .filter(|(_, ghost)| ghost.parent().is_none())
            .map(|(entity, _)| *entity));

        while let Some(entity) = self.to_visit.pop_back() {
            let mut state = match self.ghosts.remove(&entity) {
                Some(x) => x,
                None => continue,
            };
            (f)(self, entity, &mut state);
            self.ghosts.insert(entity, state);

            if let Some(kids) = self.children.get(&entity) {
                self.to_visit.extend(kids.iter().cloned());
            }
        }
    }

    fn iter_children(&self, parent: Entity) -> impl Iterator<Item = Entity> + '_ {
        struct MaybeChildren<'a>(Option<std::collections::hash_set::Iter<'a, Entity>>);

        impl<'a> Iterator for MaybeChildren<'a> {
            type Item = Entity;

            fn next(&mut self) -> Option<Self::Item> {
                self.0.as_mut().and_then(|o| o.next().copied())
            }
        }

        MaybeChildren(self.children.get(&parent).map(|c| c.iter()))
    }

    fn add_child(&mut self, parent: Entity, child: Entity) {
        self.children.entry(parent).or_default().insert(child);
    }

    fn remove_child(&mut self, parent: Entity, child: Entity) {
        remove_child(&mut self.children, parent, child);
    }

    fn set_tooltip(&mut self, entity: Entity, tooltip: Tooltip) {
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
}

fn remove_child(children: &mut HashMap<Entity, HashSet<Entity>>, parent: Entity, child: Entity) {
    if let Some(c) = children.get_mut(&parent) {
        c.remove(&child);

        if c.is_empty() {
            children.remove(&parent);
        }
    }
}

fn update_tooltip(
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

pub fn send_ghost_updates(
    entity_lookup: Res<NetEntityLookup>,
    mut clients: Query<(&NetClient, &mut ViewState)>,
) {
    for (client, mut view_state) in clients.iter_mut() {
        if !view_state.dirty {
            continue;
        }

        let possessed = view_state.possessed;
        let mut view_state = view_state.as_mut();
        view_state.dirty = false;

        view_state.for_each_removal(|entity, _| {
            let id = match entity_lookup.ecs_to_net(entity) {
                Some(x) => x,
                None => return,
            };
            client.send_packet(DeleteEntity {
                id,
            }.into());
        });

        view_state.for_each_ghost(|view_state, entity, state| {
            let id = match entity_lookup.ecs_to_net(entity) {
                Some(x) => x,
                None => return,
            };

            match state {
                GhostState::Character(character) => {
                    let dirty_flags = std::mem::replace(&mut character.dirty_flags, CharacterDirtyFlags::empty());

                    if dirty_flags.contains(CharacterDirtyFlags::UPSERT) {
                        let mut equipment = Vec::new();

                        for child_entity in view_state.iter_children(entity) {
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

                        client.send_packet(UpsertEntityCharacter {
                            id,
                            body_type: character.body_type,
                            position: character.location.position,
                            direction: character.location.direction,
                            hue: character.hue,
                            flags: character.flags,
                            notoriety: character.notoriety,
                            equipment,
                        }.into());
                    } else if dirty_flags.contains(CharacterDirtyFlags::UPDATE) {
                        client.send_packet(UpdateCharacter {
                            id,
                            body_type: character.body_type,
                            position: character.location.position,
                            direction: character.location.direction,
                            hue: character.hue,
                            flags: character.flags,
                            notoriety: character.notoriety,
                        }.into());
                    }

                    if dirty_flags.contains(CharacterDirtyFlags::STATS) {
                        client.send_packet(character.stats.upsert(id, Some(entity) == possessed).into());
                    }

                    if Some(entity) == possessed && dirty_flags.contains(CharacterDirtyFlags::UPSERT) {
                        client.send_packet(UpsertLocalPlayer {
                            id,
                            body_type: character.body_type,
                            server_id: 0,
                            hue: character.hue,
                            flags: character.flags,
                            position: character.location.position,
                            direction: character.location.direction,
                        }.into());
                    }
                }
                GhostState::Item(item) => {
                    let dirty_flags = std::mem::replace(&mut item.dirty_flags, ItemDirtyFlags::empty());

                    match &item.position {
                        ItemPositionState::World(world) => {
                            if !dirty_flags.is_empty() {
                                client.send_packet(UpsertEntityWorld {
                                    id,
                                    kind: EntityKind::Item,
                                    graphic_id: item.graphic.id,
                                    graphic_inc: 0,
                                    direction: world.location.direction,
                                    quantity: item.quantity,
                                    position: world.location.position,
                                    hue: item.graphic.hue,
                                    flags: world.flags,
                                }.into());
                            }
                        }
                        ItemPositionState::Contained(container) => {
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
                        ItemPositionState::Equipped(by) => {
                            if !dirty_flags.is_empty() {
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

                    if dirty_flags.contains(ItemDirtyFlags::TOOLTIP) && item.tooltip_version > 0 {
                        client.send_packet(EntityTooltipVersion {
                            id,
                            revision: item.tooltip_version,
                        }.into());
                    }
                }
            }
        });
    }
}

#[derive(SystemParam)]
pub struct WorldObserver<'w, 's> {
    characters: Query<'w, 's, (&'static Character, &'static Flags, &'static Location, &'static Notorious, &'static Stats)>,
    world_items: Query<'w, 's, (&'static Graphic, &'static Flags, &'static Location, Option<&'static Tooltip>, Option<&'static Quantity>, Option<&'static Container>)>,
    child_items: Query<'w, 's, (&'static Graphic, &'static ParentContainer, Option<&'static Tooltip>, Option<&'static Quantity>, Option<&'static Container>)>,
    equipped_items: Query<'w, 's, (&'static Graphic, Option<&'static Tooltip>, Option<&'static Quantity>, Option<&'static Container>)>,
}

impl<'w, 's> WorldObserver<'w, 's> {
    fn observe_container(
        &self, viewer: Entity, view_state: &mut Mut<ViewState>, container: &Container,
    ) {
        for child in &container.items {
            if let Ok((graphic, parent, tooltip, quantity, container)) = self.child_items.get(*child) {
                view_state.upsert_ghost(*child, GhostState::Item(ItemState {
                    dirty_flags: ItemDirtyFlags::empty(),
                    graphic: *graphic,
                    position: ItemPositionState::Contained(parent.clone()),
                    quantity: quantity.map_or(1, |q| q.quantity),
                    tooltip: Default::default(),
                    tooltip_version: 0,
                    container_gump: container.map(|c| c.gump_id),
                }));

                if let Some(tooltip) = tooltip {
                    update_tooltip(view_state, *child, tooltip);
                }

                if let Some(container) = container {
                    self.observe_container(viewer, view_state, container);
                }
            }
        }
    }

    fn observe_equipped(
        &self, viewer: Entity, view_state: &mut Mut<ViewState>, parent: Entity,
        entity: Entity, slot: EquipmentSlot,
        graphic: &Graphic, tooltip: Option<&Tooltip>,
        quantity: Option<&Quantity>, container: Option<&Container>,
    ) {
        view_state.upsert_ghost(entity, GhostState::Item(ItemState {
            dirty_flags: ItemDirtyFlags::empty(),
            graphic: *graphic,
            position: ItemPositionState::Equipped(EquippedBy {
                parent,
                slot,
            }),
            quantity: quantity.map_or(1, |q| q.quantity),
            tooltip: Default::default(),
            tooltip_version: 0,
            container_gump: container.map(|c| c.gump_id),
        }));

        if let Some(tooltip) = tooltip {
            update_tooltip(view_state, entity, tooltip);
        }

        if let Some(container) = container {
            self.observe_container(viewer, view_state, container);
        }
    }

    fn observe_character(
        &self, viewer: Entity, view_state: &mut Mut<ViewState>, entity: Entity,
        character: &Character, flags: EntityFlags, location: &Location, notoriety: Notoriety,
        stats: &Stats,
    ) {
        view_state.upsert_ghost(entity, GhostState::Character(CharacterState {
            dirty_flags: CharacterDirtyFlags::empty(),
            location: *location,
            body_type: character.body_type,
            hue: character.hue,
            notoriety,
            stats: stats.clone(),
            flags,
        }));

        for equipped in &character.equipment {
            if let Ok((graphic, tooltip, quantity, container)) = self.equipped_items.get(equipped.entity) {
                self.observe_equipped(
                    viewer, view_state, entity, equipped.entity, equipped.slot,
                    graphic, tooltip, quantity, container,
                );
            }
        }
    }

    fn observe_world_item(
        &self, viewer: Entity, view_state: &mut Mut<ViewState>, entity: Entity,
        graphic: &Graphic, flags: EntityFlags, location: &Location, tooltip: Option<&Tooltip>,
        quantity: Option<&Quantity>, container: Option<&Container>,
    ) {
        view_state.upsert_ghost(entity, GhostState::Item(ItemState {
            dirty_flags: ItemDirtyFlags::empty(),
            graphic: *graphic,
            position: ItemPositionState::World(WorldItemState {
                location: *location,
                flags,
            }),
            quantity: quantity.map_or(1, |q| q.quantity),
            tooltip: Default::default(),
            tooltip_version: 0,
            container_gump: container.map(|c| c.gump_id),
        }));

        if let Some(tooltip) = tooltip {
            update_tooltip(view_state, entity, tooltip);
        }

        if let Some(container) = container {
            self.observe_container(viewer, view_state, container);
        }
    }

    fn observe_entity(&self, viewer: Entity, view_state: &mut Mut<ViewState>, entity: Entity) {
        if let Ok((character, flags, location, notorious, stats)) = self.characters.get(entity) {
            self.observe_character(viewer, view_state, entity, character, flags.flags, location, notorious.0, stats);
        }

        if let Ok((graphic, flags, location, tooltip, quantity, container)) = self.world_items.get(entity) {
            self.observe_world_item(viewer, view_state, entity, graphic, flags.flags, location, tooltip, quantity, container);
        }
    }
}

pub fn observe_ghosts(
    observer: WorldObserver,
    entities: Res<EntityPositions>,
    mut clients: Query<(Entity, &View, &mut ViewState, &Possessing)>,
    owned: Query<&Location, With<NetOwner>>,
) {
    for (entity, view, mut view_state, possessing) in &mut clients {
        let location = match owned.get(possessing.entity) {
            Ok(x) => *x,
            _ => continue,
        };

        view_state.mark_unseen();

        let (min, max) = view_aabb(location.position.truncate(), view.range);
        for (visible_entity, ..) in entities.tree.iter_aabb(view_state.map_id, min, max) {
            observer.observe_entity(entity, &mut view_state, visible_entity);
        }
    }
}

pub fn start_synchronizing(
    clients: Query<(Entity, &ViewState, Ref<Possessing>), (With<NetClient>, Without<Synchronizing>)>,
    characters: Query<&Location, (With<NetOwner>, With<NetEntity>, With<Location>, With<Character>)>,
    mut commands: Commands,
) {
    for (entity, view_state, possessing) in &clients {
        let map_position = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        if !possessing.is_changed() && view_state.map_id == map_position.map_id {
            continue;
        }

        commands.entity(entity)
            .remove::<Synchronized>()
            .insert(Synchronizing);
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

pub fn send_opened_containers(
    entity_lookup: Res<NetEntityLookup>,
    clients: Query<(&NetClient, &ViewState)>,
    mut events: EventReader<ContainerOpenedEvent>,
) {
    for event in &mut events {
        let (client, view_state) = match clients.get(event.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let id = match entity_lookup.ecs_to_net(event.container) {
            Some(x) => x,
            None => continue,
        };

        let container_state = match view_state.ghosts.get(&event.container) {
            Some(GhostState::Item(x)) => x,
            _ => continue,
        };

        let gump_id = match container_state.container_gump {
            Some(x) => x,
            None => continue,
        };

        let mut contents = Vec::new();
        for child in view_state.iter_children(event.container) {
            if let Some(GhostState::Item(item)) = view_state.ghosts.get(&child) {
                let child_id = match entity_lookup.ecs_to_net(child) {
                    Some(x) => x,
                    None => continue,
                };

                let (position, grid_index) = match &item.position {
                    ItemPositionState::Contained(c) => (c.position, c.grid_index),
                    _ => continue,
                };

                contents.push(UpsertEntityContained {
                    id: child_id,
                    graphic_id: item.graphic.id,
                    graphic_inc: 0,
                    quantity: item.quantity,
                    position,
                    grid_index,
                    parent_id: id,
                    hue: item.graphic.hue,
                });
            }
        }

        client.send_packet(OpenContainer {
            id,
            gump_id,
        }.into());
        client.send_packet(UpsertContainerContents {
            contents,
        }.into());
    }
}
