use bevy_ecs::prelude::*;
use glam::IVec2;

use yewoh::{EntityId, EntityKind};
use yewoh::protocol::{CharacterEquipment, UpsertContainerContents, UpsertEntityCharacter, UpsertEntityContained, UpsertEntityWorld, UpsertLocalPlayer};

use crate::world::entity::{Character, Container, EquippedBy, Flags, Graphic, MapPosition, Notorious, ParentContainer, Quantity, Stats};
use crate::world::net::{NetClient, NetEntity, Possessing};
use crate::world::net::connection::View;
use crate::world::net::view::PartiallyVisible;
use crate::world::spatial::EntityPositions;

pub fn is_visible_to(viewer: Entity, visibility: &Option<Ref<PartiallyVisible>>) -> bool {
    visibility.as_ref().map_or(true, |v| v.is_visible_to(viewer))
}

/*
fn send_update<'a>(
    mut clients: impl Iterator<Item=(&'a NetClient, &'a CanSee, Mut<'a, HasSeen>)>,
    entity: Entity,
    update_packet_factory: impl FnOnce() -> Arc<AnyPacket>,
) {
    let mut update_packet_factory = Some(update_packet_factory);
    let mut update_packet = None;

    for (client, can_see, mut has_seen) in &mut clients {
        let can_see = can_see.entities.contains(&entity);
        if can_see {
            has_seen.entities.insert(entity);
            let packet = update_packet.get_or_insert_with(update_packet_factory.take().unwrap()).clone();
            client.send_packet_arc(packet);
        }
    }
}

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
}
 */

/*
fn send_child_items(
    client: &NetClient, possessed: Entity, sync: bool,
    id: EntityId, container: Ref<Container>,
    child_items: &Query<(
        &NetEntity, Option<Ref<PartiallyVisible>>,
        Ref<Graphic>, Ref<ParentContainer>, Option<Ref<Quantity>>, Option<Ref<Container>>,
    )>,
) {
    // If the container contents has changed, re-send the whole thing.
    // TODO: only do this for initial send
    if container.is_changed() {
        let mut items = Vec::with_capacity(container.items.len());

        for item in container.items.iter().cloned() {
            let (net_id, visibility, graphic, parent, quantity, _) = match child_items.get(item) {
                Ok(x) => x,
                _ => continue,
            };

            if !is_visible_to(possessed, &visibility) {
                continue;
            }

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

        client.send_packet(UpsertContainerContents {
            items,
        }.into());

        for item in container.items.iter().cloned() {
            let (_, visibility, _, _, _, container) = match child_items.get(item) {
                Ok(x) => x,
                _ => continue,
            };

            if !is_visible_to(possessed, &visibility) {
                continue;
            }

            if let Some(container) = container {
                send_child_items(client, possessed, sync, id, container, child_items);
            }
        }

        return;
    }

    for child_entity in container.items.iter().cloned() {
        let (net_id, visibility, graphic, parent, quantity, container) = match child_items.get(child_entity) {
            Ok(x) => x,
            _ => continue,
        };

        if (sync || visibility.as_ref().map_or(false, |r| r.is_changed()) || graphic.is_changed() || parent.is_changed() || quantity.as_ref().map_or(false, |r| r.is_changed())) && is_visible_to(possessed, &visibility) {
            client.send_packet(UpsertEntityContained {
                id: net_id.id,
                graphic_id: graphic.id,
                graphic_inc: 0,
                quantity: quantity.as_ref().map_or(1, |q| q.quantity),
                position: parent.position,
                grid_index: parent.grid_index,
                parent_id: id,
                hue: graphic.hue,
            }.into());
        }

        if let Some(container) = container {
            send_child_items(client, possessed, sync, id, container, child_items);
        }
    }
}

fn send_ghost_updates_to_client(
    client: &NetClient, view: &View, position: MapPosition, possessed: Entity, sync: bool,
    characters: &Query<(
        &NetEntity, Option<Ref<PartiallyVisible>>,
        Ref<Flags>, Ref<Character>, Ref<MapPosition>, Ref<Notorious>, Ref<Stats>,
    )>,
    world_items: &Query<(
        &NetEntity, Option<Ref<PartiallyVisible>>,
        Ref<Graphic>, Ref<Flags>, Option<Ref<Quantity>>, Ref<MapPosition>, Option<Ref<Container>>,
    )>,
    equipped_items: &Query<(&NetEntity, Ref<Graphic>, Ref<EquippedBy>)>,
    child_items: &Query<(
        &NetEntity, Option<Ref<PartiallyVisible>>,
        Ref<Graphic>, Ref<ParentContainer>, Option<Ref<Quantity>>, Option<Ref<Container>>,
    )>,
    positions: &EntityPositions,
) {
    let position2 = position.position.truncate();
    let range2 = IVec2::new(view.range as i32, view.range as i32);
    let min = position2 - range2;
    let max = position2 + range2;

    for entity in positions.tree.iter_aabb(position.map_id, min, max) {
        if let Ok((net, visibility, flags, character, position, notoriety, stats)) = characters.get(entity) {
            if (sync || flags.is_changed() || character.is_changed() || position.is_changed() || notoriety.is_changed() || stats.is_changed()) && is_visible_to(possessed, &visibility) {
                let mut equipment = Vec::new();

                for child_entity in character.equipment.iter().copied() {
                    let (net, graphic, equipped_by) = match equipped_items.get(child_entity) {
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

                client.send_packet(UpsertEntityCharacter {
                    id: net.id,
                    body_type: character.body_type,
                    position: position.position,
                    direction: position.direction,
                    hue: character.hue,
                    flags: flags.flags,
                    notoriety: notoriety.0,
                    equipment,
                }.into());

                // TODO: should this be limited to the local player?
                client.send_packet(stats.upsert(net.id, true).into());
            }
        } else if let Ok((net, visibility, graphic, flags, quantity, position, container)) = world_items.get(entity) {
            if (sync || graphic.is_changed() || flags.is_changed() || quantity.as_ref().map_or(false, |r| r.is_changed()) || position.is_changed()) && is_visible_to(possessed, &visibility) {
                client.send_packet(UpsertEntityWorld {
                    id: net.id,
                    kind: EntityKind::Item,
                    graphic_id: graphic.id,
                    graphic_inc: 0,
                    direction: position.direction,
                    quantity: quantity.map_or(1, |q| q.quantity),
                    position: position.position,
                    hue: graphic.hue,
                    flags: flags.flags,
                }.into());
            }

            if let Some(container) = container {
                send_child_items(client, possessed, sync, net.id, container, &child_items);
            }
        }
    }
}

pub fn send_ghost_updates(
    mut clients: Query<(&NetClient, &View, &Possessing), Without<NetSynchronizing>>,
    mut sync_clients: Query<(&NetClient, &View, &Possessing), With<NetSynchronizing>>,
    characters: Query<(
        &NetEntity, Option<Ref<PartiallyVisible>>,
        Ref<Flags>, Ref<Character>, Ref<MapPosition>, Ref<Notorious>, Ref<Stats>,
    )>,
    world_items: Query<(
        &NetEntity, Option<Ref<PartiallyVisible>>,
        Ref<Graphic>, Ref<Flags>, Option<Ref<Quantity>>, Ref<MapPosition>, Option<Ref<Container>>,
    )>,
    equipped_items: Query<(&NetEntity, Ref<Graphic>, Ref<EquippedBy>)>,
    child_items: Query<(
        &NetEntity, Option<Ref<PartiallyVisible>>,
        Ref<Graphic>, Ref<ParentContainer>, Option<Ref<Quantity>>, Option<Ref<Container>>,
    )>,
    positions: Res<EntityPositions>,
) {
    for (client, view, possessing) in clients.iter_mut() {
        let (net, _, flags, character, position, ..) = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        if character.is_changed() || flags.is_changed() || position.is_changed() {
            client.send_packet(UpsertLocalPlayer {
                id: net.id,
                body_type: character.body_type,
                server_id: 0,
                hue: character.hue,
                flags: flags.flags,
                position: position.position,
                direction: position.direction,
            }.into());
        }

        send_ghost_updates_to_client(client, view, *position, possessing.entity, false, &characters, &world_items, &equipped_items, &child_items, &positions);
    }

    for (client, view, possessing) in sync_clients.iter_mut() {
        let (net, _, flags, character, position, ..) = match characters.get(possessing.entity) {
            Ok(x) => x,
            _ => continue,
        };

        client.send_packet(UpsertLocalPlayer {
            id: net.id,
            body_type: character.body_type,
            server_id: 0,
            hue: character.hue,
            flags: flags.flags,
            position: position.position,
            direction: position.direction,
        }.into());

        send_ghost_updates_to_client(client, view, *position, possessing.entity, true, &characters, &world_items, &equipped_items, &child_items, &positions);
    }
}
*/
/*
pub fn sync_entities(
    mut clients: Query<(&NetClient, &CanSee, &mut HasSeen), With<NetSynchronizing>>,
    characters: Query<(Entity, &NetEntity, &CharacterState)>,
    world_items: Query<(Entity, &NetEntity, &WorldItemState)>,
    contained_items: Query<(Entity, &NetEntity, &ContainedItemState)>,
    equipped_items: Query<(Entity, &NetEntity, &EquippedItemState)>,
    stats: Query<(Entity, &NetEntity, &Stats)>,
    tooltips: Query<(Entity, &NetEntity, Ref<Tooltip>), With<Tooltip>>,
    all_equipment_query: Query<(&NetEntity, &Graphic, &EquippedBy)>,
) {
    if clients.is_empty() {
        return;
    }

    for (entity, net, state) in characters.iter() {
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id, &all_equipment_query).into_arc());
    }

    for (entity, net, state) in equipped_items.iter() {
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id).into_arc());
    }

    for (entity, net, state) in world_items.iter() {
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id).into_arc());
    }

    for (entity, net, state) in contained_items.iter() {
        send_update(
            clients.iter_mut(),
            entity,
            || state.to_update(net.id).into_arc());
    }

    for (entity, net, stats) in stats.iter() {
        send_update(
            clients.iter_mut(),
            entity,
            || stats.upsert(net.id, true).into_arc());
    }

    for (entity, net, tooltip) in tooltips.iter() {
        send_update(
            clients.iter_mut(),
            entity,
            || EntityTooltipVersion {
                id: net.id,
                revision: tooltip.last_changed(),
            }.into_arc());
    }
}

*/
