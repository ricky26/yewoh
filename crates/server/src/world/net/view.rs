use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::{Changed, With, Without};
use bevy_ecs::system::{Commands, Query, Res, Resource};
use bevy_ecs::world::{Mut, Ref};
use bevy_reflect::Reflect;
use glam::IVec2;
use glam::UVec2;

use bitflags::bitflags;
use yewoh::{EntityId, EntityKind, Notoriety};
use yewoh::protocol::{AnyPacket, CharacterEquipment, DeleteEntity, EntityFlags, EntityTooltipVersion, EquipmentSlot, UpsertEntityCharacter, UpsertEntityContained, UpsertEntityEquipped, UpsertEntityWorld, UpsertLocalPlayer};
use yewoh::protocol::{BeginEnterWorld, ChangeSeason, EndEnterWorld, ExtendedCommand};

use crate::world::entity::{Character, EquippedBy, Graphic, MapPosition, ParentContainer, Tooltip};
use crate::world::net::{NetClient, NetEntity, NetEntityLookup};
use crate::world::net::connection::Possessing;
use crate::world::spatial::NetClientPositions;

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
    pub map_id: u8,
    pub range: u32,
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
        const ALL = 1 << 0;
        const REMOVE = 1 << 1;
        const MOVE = 1 << 2;
    }

    #[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
    pub struct ItemDirtyFlags : u8 {
        const ALL = 1 << 0;
        const REMOVE = 1 << 1;
        const MOVE = 1 << 2;
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CharacterState {
    pub dirty_flags: CharacterDirtyFlags,
    pub position: MapPosition,
    pub character: Character,
    pub notoriety: Notoriety,
    pub flags: EntityFlags,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorldItemState {
    pub position: MapPosition,
    pub flags: EntityFlags,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum GhostState {
    Character(CharacterState),
    Item(ItemState),
}

#[derive(Default, Debug, Component)]
pub struct ViewState {
    ghosts: HashMap<Entity, GhostState>,
    position: Option<MapPosition>,
    possessed: Option<Entity>,
    dirty: bool,
}

impl ViewState {
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn flush(&mut self) {
        self.ghosts.clear();
        self.dirty = false;
    }
}

pub fn is_visible_to(viewer: Entity, visibility: &Option<Ref<PartiallyVisible>>) -> bool {
    visibility.as_ref().map_or(true, |v| v.is_visible_to(viewer))
}

pub fn set_view_position(view_state: &mut Mut<ViewState>, position: MapPosition) {
    if view_state.position != Some(position) {
        view_state.position = Some(position);
    }
}

pub fn add_ghost(
    view_state: &mut Mut<ViewState>, entity: Entity, state: GhostState,
) {
    let previous = view_state.ghosts.remove(&entity);

    match (previous, &state) {
        (Some(GhostState::Character(old)), GhostState::Character(new)) => {}
        (old, GhostState::Character(new)) => {
            if Some(entity) == view_state.possessed {
                view_state.position = Some(new.position);
            }
        }
        _ => {}
    }
    view_state.ghosts.insert(entity, state);
}

pub fn remove_ghost(view_state: &mut Mut<ViewState>, entity: Entity) {
    if !view_state.ghosts.contains_key(&entity) {
        return;
    }

    match view_state.ghosts.get_mut(&entity).unwrap() {
        GhostState::Character(character) =>
            character.dirty_flags |= CharacterDirtyFlags::REMOVE,
        GhostState::Item(item) =>
            item.dirty_flags |= ItemDirtyFlags::REMOVE,
    }
}

pub fn set_tooltip(
    view_state: &mut Mut<ViewState>, entity: Entity, tooltip: &Tooltip,
) {
    match view_state.ghosts.get(&entity) {
        Some(GhostState::Item(item)) => {
            if &item.tooltip == tooltip {
                return;
            }
        },
        _ => return,
    }

    match view_state.ghosts.get_mut(&entity) {
        Some(GhostState::Item(item)) => {
            item.tooltip = tooltip.clone();
        }
        _ => unreachable!(),
    }
}

pub fn update_tooltips(
    mut clients: Query<&mut ViewState>,
    tooltips: Query<(Entity, Ref<Tooltip>), Changed<Tooltip>>,
) {
    for (entity, tooltip) in tooltips.iter() {
        for mut view_state in clients.iter_mut() {
            set_tooltip(&mut view_state, entity, &tooltip);
        }
    }
}

fn send_child_ghost_updates() {

}

pub fn send_ghost_updates(mut clients: Query<(&NetClient, &mut ViewState), Changed<ViewState>>) {
    for (client, mut view_state) in clients.iter_mut() {
        if !view_state.dirty {
            continue;
        }

        let mut view_state = view_state.as_mut();
        view_state.dirty = false;

        /*for packet in view_state.queued_packets.drain(..) {
            client.send_packet_arc(packet);
        }*/
    }
}


/*
pub fn update_players(
    clients: Query<&NetClient>,
    added: Query<
        (Entity, &NetEntity, &NetOwner, &Flags, &Character, &MapPosition),
        Without<PlayerState>,
    >,
    mut updated: Query<
        (&mut PlayerState, &NetEntity, &NetOwner, &Flags, &Character, &MapPosition),
        Or<(Changed<Character>, Changed<MapPosition>)>,
    >,
    removed: Query<
        Entity,
        (With<PlayerState>, Or<(Without<NetOwner>, Without<Flags>, Without<Character>, Without<MapPosition>)>),
    >,
    mut commands: Commands,
) {
    for (entity, net, owner, flags, character, position) in added.iter() {
        let client = match clients.get(owner.client_entity) {
            Ok(x) => x,
            _ => continue,
        };
        let state = PlayerState {
            character: character.clone(),
            flags: flags.flags,
            position: position.clone(),
        };

        client.send_packet(state.to_update(net.id).into());
        commands.entity(entity).insert(state);
    }

    for (mut state, net, owner, flags, character, position) in updated.iter_mut() {
        let client = match clients.get(owner.client_entity) {
            Ok(x) => x,
            _ => continue,
        };
        let new_state = PlayerState {
            character: character.clone(),
            flags: flags.flags,
            position: position.clone(),
        };
        if new_state == *state {
            continue;
        }
        *state = new_state;
        client.send_packet(state.to_update(net.id).into());
    }

    for entity in removed.iter() {
        commands.entity(entity).remove::<PlayerState>();
    }
}

pub fn update_items_in_world(
    mut clients: Query<(&NetClient, &CanSee, &mut HasSeen), With<NetSynchronized>>,
    new_items: Query<
        (Entity, &NetEntity, &Flags, &Graphic, &MapPosition, Option<&Quantity>),
        Without<WorldItemState>,
    >,
    mut updated_items: Query<
        (Entity, &mut WorldItemState, &NetEntity, &Flags, &Graphic, &MapPosition, Option<&Quantity>),
        Or<(Changed<Graphic>, Changed<MapPosition>, Changed<Quantity>)>,
    >,
    removed_items: Query<
        Entity,
        (With<WorldItemState>, Or<(Without<Flags>, Without<Graphic>, Without<MapPosition>)>),
    >,
    mut commands: Commands,
) {
    for (entity, net, flags, graphic, position, quantity) in new_items.iter() {
        let position = *position;
        let graphic = *graphic;
        let quantity = quantity.map_or(1, |q| q.quantity);
        let state = WorldItemState {
            position,
            graphic,
            quantity,
            flags: flags.flags,
        };
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id).into_arc());
        commands.entity(entity).insert(state);
    }

    for (entity, mut state, net, flags, graphic, position, quantity) in updated_items.iter_mut() {
        let graphic = *graphic;
        let position = *position;
        let quantity = quantity.map_or(1, |q| q.quantity);
        let new_state = WorldItemState {
            position,
            graphic,
            quantity,
            flags: flags.flags,
        };
        if new_state == *state {
            continue;
        }
        *state = new_state;
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id).into_arc());
    }

    for entity in removed_items.iter() {
        commands.entity(entity).remove::<WorldItemState>();
    }
}

pub fn update_items_in_containers(
    mut clients: Query<(&NetClient, &CanSee, &mut HasSeen), With<NetSynchronized>>,
    net_entities: Query<&NetEntity>,
    new_items: Query<
        (Entity, &NetEntity, &Graphic, &ParentContainer, Option<&Quantity>),
        Without<ContainedItemState>,
    >,
    mut updated_items: Query<
        (Entity, &mut ContainedItemState, &NetEntity, &Graphic, &ParentContainer, Option<&Quantity>),
        Or<(Changed<Graphic>, Changed<ParentContainer>, Changed<Quantity>)>,
    >,
    removed_items: Query<
        Entity,
        (With<WorldItemState>, Or<(Without<Graphic>, Without<ParentContainer>)>),
    >,
    mut commands: Commands,
) {
    for (entity, net, graphic, parent, quantity) in new_items.iter() {
        let parent_id = match net_entities.get(parent.parent) {
            Ok(x) => x.id,
            _ => continue,
        };
        let graphic = *graphic;
        let quantity = quantity.map_or(1, |q| q.quantity);
        let state = ContainedItemState {
            parent_id,
            graphic,
            position: parent.position,
            grid_index: parent.grid_index,
            quantity,
        };
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id).into_arc());
        commands.entity(entity).insert(state);
    }

    for (entity, mut state, net, graphic, parent, quantity) in updated_items.iter_mut() {
        let parent_id = match net_entities.get(parent.parent) {
            Ok(x) => x.id,
            _ => continue,
        };
        let graphic = *graphic;
        let quantity = quantity.map_or(1, |q| q.quantity);
        let new_state = ContainedItemState {
            parent_id,
            graphic,
            position: parent.position,
            grid_index: parent.grid_index,
            quantity,
        };
        if new_state == *state {
            continue;
        }
        *state = new_state;
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id).into_arc());
    }

    for entity in removed_items.iter() {
        commands.entity(entity).remove::<ContainedItemState>();
    }
}

pub fn update_equipped_items(
    mut clients: Query<(&NetClient, &CanSee, &mut HasSeen), With<NetSynchronized>>,
    net_entities: Query<&NetEntity>,
    new_items: Query<
        (Entity, &NetEntity, &Graphic, &EquippedBy),
        Without<EquippedItemState>,
    >,
    mut updated_items: Query<
        (Entity, &mut EquippedItemState, &NetEntity, &Graphic, &EquippedBy),
        Or<(Changed<Graphic>, Changed<EquippedBy>)>,
    >,
    removed_items: Query<
        Entity,
        (With<EquippedItemState>, Or<(Without<Graphic>, Without<EquippedBy>)>),
    >,
    mut commands: Commands,
) {
    for (entity, net, graphic, equipped) in new_items.iter() {
        let parent_id = match net_entities.get(equipped.parent) {
            Ok(x) => x.id,
            _ => continue,
        };
        let graphic = *graphic;
        let state = EquippedItemState {
            parent_id,
            slot: equipped.slot,
            graphic,
        };
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id).into_arc());
        commands.entity(entity).insert(state);
    }

    for (entity, mut state, net, graphic, equipped) in updated_items.iter_mut() {
        let parent_id = match net_entities.get(equipped.parent) {
            Ok(x) => x.id,
            _ => continue,
        };
        let graphic = *graphic;
        let new_state = EquippedItemState {
            parent_id,
            slot: equipped.slot,
            graphic,
        };
        if new_state == *state {
            continue;
        }
        *state = new_state;
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id).into_arc());
    }

    for entity in removed_items.iter() {
        commands.entity(entity).remove::<EquippedItemState>();
    }
}

pub fn update_characters(
    mut clients: Query<(&NetClient, &CanSee, &mut HasSeen), With<NetSynchronized>>,
    new_characters: Query<
        (Entity, &NetEntity, &Flags, &Character, &MapPosition, &Notorious),
        Without<CharacterState>,
    >,
    mut updated_characters: Query<
        (Entity, &mut CharacterState, &NetEntity, &Flags, &Character, &MapPosition, &Notorious),
        Or<(Changed<Character>, Changed<MapPosition>, Changed<Notorious>)>,
    >,
    removed_characters: Query<
        Entity,
        (With<CharacterState>, Or<(Without<Flags>, Without<Character>, Without<MapPosition>, Without<Notorious>)>),
    >,
    all_equipment_query: Query<(&NetEntity, &Graphic, &EquippedBy)>,
    mut commands: Commands,
) {
    for (entity, net, flags, character, position, notorious) in new_characters.iter() {
        let character = character.clone();
        let position = *position;
        let notoriety = notorious.0;
        let state = CharacterState {
            position,
            character,
            notoriety,
            flags: flags.flags,
        };
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id, &all_equipment_query).into_arc());
        commands.entity(entity).insert(state);
    }

    for (entity, mut state, net, flags, character, position, notorious) in updated_characters.iter_mut() {
        let character = character.clone();
        let position = *position;
        let notoriety = notorious.0;
        let new_state = CharacterState {
            position,
            character,
            notoriety,
            flags: flags.flags,
        };
        if *state == new_state {
            continue;
        }
        *state = new_state;
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id, &all_equipment_query).into_arc());
    }

    for entity in removed_characters.iter() {
        commands.entity(entity).remove::<CharacterState>();
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

pub fn send_hidden_entities(
    lookup: Res<NetEntityLookup>,
    mut clients: Query<(&NetClient, &CanSee, &mut HasSeen), Changed<CanSee>>,
) {
    for (client, can_see, mut has_seen) in &mut clients {
        let to_remove = has_seen.entities.difference(&can_see.entities)
            .cloned()
            .collect::<Vec<_>>();
        for entity in to_remove {
            has_seen.entities.remove(&entity);

            if let Some(id) = lookup.ecs_to_net(entity) {
                client.send_packet(DeleteEntity { id }.into());
            }
        }
    }
}

pub fn send_remove_entity(
    lookup: Res<NetEntityLookup>,
    mut clients: Query<(&NetClient, &mut HasSeen)>,
    mut removals: RemovedComponents<NetEntity>,
) {
    for entity in removals.iter() {
        let id = match lookup.ecs_to_net(entity) {
            Some(x) => x,
            None => continue,
        };

        let mut packet = None;
        for (client, mut has_seen) in &mut clients {
            if !has_seen.entities.contains(&entity) {
                continue;
            }

            has_seen.entities.remove(&entity);
            let packet = packet.get_or_insert_with(|| DeleteEntity { id }.into_arc()).clone();
            client.send_packet_arc(packet);
        }
    }
}

pub fn send_updated_stats(
    mut clients: Query<(&NetClient, &CanSee, &mut HasSeen), With<NetSynchronized>>,
    query: Query<(Entity, &NetEntity, &Stats), Changed<Stats>>,
) {
    for (entity, net, stats) in &query {
        send_update(
            clients.iter_mut(),
            entity,
            || stats.upsert(net.id, true).into_arc());
    }
}*/

pub fn start_synchronizing(
    clients: Query<(Entity, &View, Ref<Possessing>), Without<Synchronizing>>,
    characters: Query<&MapPosition>,
    mut commands: Commands,
) {
    for (entity, view, possessing) in clients.iter() {
        let map_position = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        if !possessing.is_changed() && view.map_id == map_position.map_id {
            continue;
        }

        commands.entity(entity).insert(Synchronizing);
    }
}

pub fn send_change_map(
    mut clients: Query<(&NetClient, &mut View, &mut ViewState, Ref<Possessing>), With<Synchronizing>>,
    characters: Query<(&NetEntity, &MapPosition, Ref<Character>)>,
    maps: Res<MapInfos>,
) {
    for (client, mut view, mut view_state, possessing) in clients.iter_mut() {
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
        } else if view.map_id == map_position.map_id {
            continue;
        }

        view_state.flush();
        view.map_id = map_position.map_id;
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

/*
impl CharacterState {
    fn to_update(
        &self,
        id: EntityId,
        all_equipment_query: &Query<(&NetEntity, &Graphic, &EquippedBy)>,
    ) -> UpsertEntityCharacter {
        let mut equipment = Vec::new();

        for child_entity in self.character.equipment.iter().copied() {
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

        UpsertEntityCharacter {
            id,
            body_type: self.character.body_type,
            position: self.position.position,
            direction: self.position.direction,
            hue: self.character.hue,
            flags: self.flags,
            notoriety: self.notoriety,
            equipment,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PlayerState {
    pub character: Character,
    pub flags: EntityFlags,
    pub position: MapPosition,
}

impl PlayerState {
    fn to_update(&self, id: EntityId) -> UpsertLocalPlayer {
        UpsertLocalPlayer {
            id,
            body_type: self.character.body_type,
            server_id: 0,
            hue: self.character.hue,
            flags: self.flags,
            position: self.position.position,
            direction: self.position.direction,
        }
    }
}
 */

/*
impl WorldItemState {
    fn to_update(&self, id: EntityId) -> UpsertEntityWorld {
        UpsertEntityWorld {
            id,
            kind: EntityKind::Item,
            graphic_id: self.graphic.id,
            graphic_inc: 0,
            direction: self.position.direction,
            quantity: self.quantity,
            position: self.position.position,
            hue: self.graphic.hue,
            flags: self.flags,
        }
    }
}
 */

/*
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContainedItemState {
    pub parent: ParentContainer,
}

impl ContainedItemState {
    fn to_update(&self, id: EntityId) -> UpsertEntityContained {
        UpsertEntityContained {
            id,
            graphic_id: self.graphic.id,
            graphic_inc: 0,
            quantity: self.quantity,
            position: self.position,
            grid_index: self.grid_index,
            parent_id: self.parent_id,
            hue: self.graphic.hue,
        }
    }
}
 */

/*
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EquippedItemState {
    pub equipped: EquippedBy,
}

impl EquippedItemState {
    fn to_update(&self, id: EntityId) -> UpsertEntityEquipped {
        UpsertEntityEquipped {
            id,
            parent_id: self.parent_id,
            slot: self.slot,
            graphic_id: self.graphic.id,
            hue: self.graphic.hue,
        }
    }
}
 */
