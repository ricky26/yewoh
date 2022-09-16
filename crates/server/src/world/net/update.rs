use bevy_ecs::prelude::*;
use glam::IVec2;

use yewoh::{EntityId, EntityKind, Notoriety};
use yewoh::protocol::{CharacterEquipment, DeleteEntity, EntityFlags, EntityTooltipVersion, EquipmentSlot, Packet, UpsertContainerContents, UpsertEntityCharacter, UpsertEntityContained, UpsertEntityEquipped, UpsertEntityWorld, UpsertLocalPlayer};

use crate::world::entity::{Character, Container, EquippedBy, Flags, Graphic, MapPosition, Notorious, ParentContainer, Quantity, Stats, Tooltip};
use crate::world::net::{broadcast, NetClient, NetEntity, NetEntityLookup, NetOwner};
use crate::world::net::owner::NetSynchronizing;
use crate::world::time::Tick;

#[derive(Debug, Clone, Eq, PartialEq, Component)]
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

#[derive(Debug, Clone, Eq, PartialEq, Component)]
pub struct WorldItemState {
    pub position: MapPosition,
    pub graphic: Graphic,
    pub quantity: u16,
    pub flags: EntityFlags,
}

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

pub fn update_items_in_world(
    clients: Query<&NetClient>,
    new_items: Query<
        (Entity, &NetEntity, &Flags, &Graphic, &MapPosition, Option<&Quantity>),
        Without<WorldItemState>,
    >,
    mut updated_items: Query<
        (&mut WorldItemState, &NetEntity, &Flags, &Graphic, &MapPosition, Option<&Quantity>),
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
        broadcast(clients.iter(), state.to_update(net.id).into_arc());
        commands.entity(entity).insert(state);
    }

    for (mut state, net, flags, graphic, position, quantity) in updated_items.iter_mut() {
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
        broadcast(clients.iter(), state.to_update(net.id).into_arc());
    }

    for entity in removed_items.iter() {
        commands.entity(entity).remove::<WorldItemState>();
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Component)]
pub struct ContainedItemState {
    pub parent_id: EntityId,
    pub graphic: Graphic,
    pub position: IVec2,
    pub grid_index: u8,
    pub quantity: u16,
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

pub fn update_items_in_containers(
    clients: Query<&NetClient>,
    net_entities: Query<&NetEntity>,
    new_items: Query<
        (Entity, &NetEntity, &Graphic, &ParentContainer, Option<&Quantity>),
        Without<ContainedItemState>,
    >,
    mut updated_items: Query<
        (&mut ContainedItemState, &NetEntity, &Graphic, &ParentContainer, Option<&Quantity>),
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
        broadcast(clients.iter(), state.to_update(net.id).into_arc());
        commands.entity(entity).insert(state);
    }

    for (mut state, net, graphic, parent, quantity) in updated_items.iter_mut() {
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
        broadcast(clients.iter(), state.to_update(net.id).into_arc());
    }

    for entity in removed_items.iter() {
        commands.entity(entity).remove::<ContainedItemState>();
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Component)]
pub struct EquippedItemState {
    pub parent_id: EntityId,
    pub slot: EquipmentSlot,
    pub graphic: Graphic,
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

pub fn update_equipped_items(
    clients: Query<&NetClient>,
    net_entities: Query<&NetEntity>,
    new_items: Query<
        (Entity, &NetEntity, &Graphic, &EquippedBy),
        Without<EquippedItemState>,
    >,
    mut updated_items: Query<
        (&mut EquippedItemState, &NetEntity, &Graphic, &EquippedBy),
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
        broadcast(clients.iter(), state.to_update(net.id).into_arc());
        commands.entity(entity).insert(state);
    }

    for (mut state, net, graphic, equipped) in updated_items.iter_mut() {
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
        broadcast(clients.iter(), state.to_update(net.id).into_arc());
    }

    for entity in removed_items.iter() {
        commands.entity(entity).remove::<EquippedItemState>();
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Component)]
pub struct CharacterState {
    pub position: MapPosition,
    pub character: Character,
    pub notoriety: Notoriety,
    pub flags: EntityFlags,
}

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

pub fn update_characters(
    clients: Query<&NetClient>,
    new_characters: Query<
        (Entity, &NetEntity, &Flags, &Character, &MapPosition, &Notorious),
        Without<CharacterState>,
    >,
    mut updated_characters: Query<
        (&mut CharacterState, &NetEntity, &Flags, &Character, &MapPosition, &Notorious),
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
        broadcast(clients.iter(), state.to_update(net.id, &all_equipment_query).into_arc());
        commands.entity(entity).insert(state);
    }

    for (mut state, net, flags, character, position, notorious) in updated_characters.iter_mut() {
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
        broadcast(clients.iter(), state.to_update(net.id, &all_equipment_query).into_arc());
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

pub fn sync_entities(
    tick: Res<Tick>,
    clients: Query<&NetClient, With<NetSynchronizing>>,
    characters: Query<(&NetEntity, &CharacterState)>,
    world_items: Query<(&NetEntity, &WorldItemState)>,
    contained_items: Query<(&NetEntity, &ContainedItemState)>,
    equipped_items: Query<(&NetEntity, &EquippedItemState)>,
    stats: Query<(&NetEntity, &Stats)>,
    tooltips: Query<&NetEntity, With<Tooltip>>,
    all_equipment_query: Query<(&NetEntity, &Graphic, &EquippedBy)>,
) {
    for (net, state) in characters.iter() {
        broadcast(clients.iter(), state.to_update(net.id, &all_equipment_query).into_arc());
    }

    for (net, state) in equipped_items.iter() {
        broadcast(clients.iter(), state.to_update(net.id).into_arc());
    }

    for (net, state) in world_items.iter() {
        broadcast(clients.iter(), state.to_update(net.id).into_arc());
    }

    for (net, state) in contained_items.iter() {
        broadcast(clients.iter(), state.to_update(net.id).into_arc());
    }

    for (net, stats) in stats.iter() {
        broadcast(clients.iter(), stats.upsert(net.id, true).into_arc());
    }

    for net in tooltips.iter() {
        broadcast(clients.iter(), EntityTooltipVersion {
            id: net.id,
            revision: tick.tick,
        }.into_arc());
    }
}

pub fn update_tooltips(
    tick: Res<Tick>,
    clients: Query<&NetClient>,
    tooltips: Query<&NetEntity, Changed<Tooltip>>,
) {
    let tick = tick.tick;
    for net in tooltips.iter() {
        broadcast(clients.iter(), EntityTooltipVersion {
            id: net.id,
            revision: tick,
        }.into_arc());
    }
}