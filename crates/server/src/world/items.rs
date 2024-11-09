use bevy::ecs::entity::EntityHashMap;
use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::prelude::*;
use bevy::utils::hashbrown::hash_map::Entry;
use std::collections::VecDeque;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use yewoh::protocol::{AnyPacket, DeleteEntity, EntityFlags, EntityTooltipVersion, IntoAnyPacket, UpsertEntityContained, UpsertEntityEquipped, UpsertEntityWorld};
use yewoh::{EntityId, EntityKind};

use crate::world::delta_grid::{delta_grid_cell, DeltaEntry, DeltaGrid, DeltaVersion};
use crate::world::entity::{ContainedPosition, EquippedPosition, Hue, MapPosition, RootPosition, Tooltip};
use crate::world::map::Static;
use crate::world::net_id::{OnDestroyNetEntity, NetId};
use crate::world::ServerSet;

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Deref, DerefMut, Component, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[serde(transparent)]
#[require(Hue, ItemGraphicOffset, ItemQuantity, Tooltip, RootPosition)]
pub struct ItemGraphic(pub u16);

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Deref, DerefMut, Component, Reflect)]
#[reflect(Component, Default)]
pub struct ItemGraphicOffset(pub u8);

#[derive(Debug, Clone, Eq, PartialEq, Component, Reflect, Deref, DerefMut)]
#[reflect(Component, Default)]
pub struct ItemQuantity(pub u16);

impl Default for ItemQuantity {
    fn default() -> Self {
        ItemQuantity(1)
    }
}

#[derive(Debug, Clone, Copy, Default, Component, Reflect)]
#[reflect(Component, Default)]
pub struct Container {
    pub gump_id: u16,
}

#[derive(Debug, Clone, Event)]
pub struct OnContainerOpen {
    pub client_entity: Entity,
    pub container: Entity,
}

#[derive(Clone, Debug, )]
pub enum ItemPosition {
    Map(MapPosition),
    Equipped(Entity, EquippedPosition),
    Contained(Entity, ContainedPosition),
}

#[derive(QueryData)]
pub struct PositionQuery {
    pub parent: Option<Ref<'static, Parent>>,
    pub map: Option<Ref<'static, MapPosition>>,
    pub equipped: Option<Ref<'static, EquippedPosition>>,
    pub contained: Option<Ref<'static, ContainedPosition>>,
}

impl PositionQueryItem<'_> {
    pub fn item_position(&self) -> Option<ItemPosition> {
        if let Some(parent) = self.parent.as_ref() {
            self.equipped.as_ref()
                .map(|e| ItemPosition::Equipped(parent.get(), **e))
                .or_else(|| self.contained.as_ref()
                    .map(|c| ItemPosition::Contained(parent.get(), **c)))
        } else {
            self.map.as_ref()
                .map(|m| ItemPosition::Map(**m))
        }
    }

    pub fn is_added(&self) -> bool {
        if let Some(parent) = self.parent.as_ref() {
            if parent.is_added() {
                true
            } else if let Some(equipped) = self.equipped.as_ref() {
                equipped.is_added()
            } else if let Some(contained) = self.contained.as_ref() {
                contained.is_added()
            } else {
                false
            }
        } else if let Some(map) = self.map.as_ref() {
            map.is_added()
        } else {
            false
        }
    }

    pub fn is_changed(&self) -> bool {
        if let Some(parent) = self.parent.as_ref() {
            if parent.is_changed() {
                true
            } else if let Some(equipped) = self.equipped.as_ref() {
                equipped.is_changed()
            } else if let Some(contained) = self.contained.as_ref() {
                contained.is_changed()
            } else {
                false
            }
        } else if let Some(map) = self.map.as_ref() {
            map.is_changed()
        } else {
            false
        }
    }
}

#[derive(QueryFilter)]
pub struct ChangedPositionFilter {
    _query: Or<(
        Changed<Parent>,
        Changed<MapPosition>,
        Changed<EquippedPosition>,
        Changed<ContainedPosition>,
    )>,
}

#[derive(QueryFilter)]
pub struct ValidItemPosition {
    _one_of: Or<(
        (Without<Parent>, With<MapPosition>),
        (With<Parent>, Or<(With<EquippedPosition>, With<ContainedPosition>)>),
    )>,
}

#[derive(QueryData)]
pub struct ItemQuery {
    pub graphic: Ref<'static, ItemGraphic>,
    pub graphic_offset: Ref<'static, ItemGraphicOffset>,
    pub hue: Ref<'static, Hue>,
    pub quantity: Ref<'static, ItemQuantity>,
    pub tooltip: Ref<'static, Tooltip>,
    pub position: PositionQuery,
}

impl ItemQueryItem<'_> {
    pub fn flags(&self) -> EntityFlags {
        EntityFlags::empty()
    }

    pub fn parent(&self) -> Option<Entity> {
        self.position.parent.as_ref().map(|p| p.get())
    }

    pub fn to_upsert_map(
        &self, id: EntityId,
    ) -> Option<UpsertEntityWorld> {
        let position = self.position.map.as_ref()?;
        Some(UpsertEntityWorld {
            id,
            kind: EntityKind::Item,
            graphic_id: **self.graphic,
            graphic_inc: **self.graphic_offset,
            direction: position.direction,
            quantity: **self.quantity,
            position: position.position,
            hue: **self.hue,
            flags: self.flags(),
        })
    }

    pub fn to_upsert_equipped(
        &self, id: EntityId, parent_id: EntityId,
    ) -> Option<UpsertEntityEquipped> {
        let equipped = self.position.equipped.as_ref()?;
        Some(UpsertEntityEquipped {
            id,
            parent_id,
            slot: equipped.slot,
            graphic_id: **self.graphic,
            hue: **self.hue,
        })
    }

    pub fn to_upsert_contained(
        &self, id: EntityId, parent_id: EntityId,
    ) -> Option<UpsertEntityContained> {
        let contained = self.position.contained.as_ref()?;
        Some(UpsertEntityContained {
            id,
            graphic_id: **self.graphic,
            graphic_inc: **self.graphic_offset,
            quantity: **self.quantity,
            position: contained.position,
            grid_index: contained.grid_index,
            parent_id,
            hue: **self.hue,
        })
    }

    pub fn to_upsert(&self, id: EntityId, parent_id: Option<EntityId>) -> Option<AnyPacket> {
        let item_position = self.position.item_position()?;
        let packet = match item_position {
            ItemPosition::Map(_) =>
                AnyPacket::from(self.to_upsert_map(id)?),
            ItemPosition::Equipped(_, _) =>
                AnyPacket::from(self.to_upsert_equipped(id, parent_id?)?),
            ItemPosition::Contained(_, _) =>
                AnyPacket::from(self.to_upsert_contained(id, parent_id?)?),
        };
        Some(packet)
    }

    pub fn is_added(&self) -> bool {
        self.graphic.is_added() ||
            self.position.is_added()
    }

    pub fn is_changed(&self) -> bool {
        self.is_item_changed() ||
            self.tooltip.is_changed() ||
            self.position.is_changed()
    }

    pub fn is_item_changed(&self) -> bool {
        self.graphic.is_changed() ||
            self.graphic_offset.is_changed() ||
            self.hue.is_changed() ||
            self.quantity.is_changed()
    }
}

#[derive(QueryData)]
pub struct ContainerQuery {
    container: &'static Container,
    children: Option<&'static Children>,
}

#[derive(QueryFilter)]
pub struct ChangedItemFilter {
    _query: Or<(
        Changed<ItemGraphic>,
        Changed<Hue>,
        Changed<ItemQuantity>,
        Changed<Tooltip>,
    )>,
}

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct StaticRootPositionSet;

pub fn set_static_root_positions(
    mut commands: Commands,
    mut root_query: Query<(Entity, &MapPosition, &mut RootPosition), (With<Static>, Without<StaticRootPositionSet>, Without<Parent>)>,
) {
    for (entity, position, mut root) in &mut root_query {
        commands.entity(entity).insert(StaticRootPositionSet);
        root.0 = *position;
    }
}

pub fn update_root_position(
    root_query: Query<
        (Entity, &MapPosition),
        (Without<Static>, With<RootPosition>, Without<Parent>, Changed<MapPosition>),
    >,
    child_query: Query<
        (Entity, &Parent),
        (Without<Static>, With<RootPosition>, Changed<Parent>),
    >,
    mut any_query: Query<(&mut RootPosition, Option<&Children>), Without<Static>>,
    mut child_queue: Local<VecDeque<(Entity, MapPosition)>>,
) {
    child_queue.extend(root_query.iter().map(|(e, m)| (e, *m)));
    update_root_position_inner(&mut any_query, &mut child_queue);

    child_queue.extend(child_query.iter()
        .filter_map(|(entity, parent)| {
            let (root, _) = any_query.get(parent.get()).ok()?;
            Some((entity, root.0))
        }));
    update_root_position_inner(&mut any_query, &mut child_queue);
}

fn update_root_position_inner(
    any_query: &mut Query<(&mut RootPosition, Option<&Children>), Without<Static>>,
    child_queue: &mut VecDeque<(Entity, MapPosition)>,
) {
    while let Some((entity, position)) = child_queue.pop_front() {
        let Ok((mut root, children)) = any_query.get_mut(entity) else {
            continue;
        };

        root.0 = position;

        if let Some(children) = children {
            child_queue.extend(
                children.iter().map(|e| (*e, position)));
        }
    }
}

#[derive(Default)]
pub struct ItemCache {
    pub last_position: EntityHashMap<MapPosition>,
}

pub fn detect_item_changes(
    mut cache: Local<ItemCache>,
    delta_version: Res<DeltaVersion>,
    mut delta_grid: ResMut<DeltaGrid>,
    changed_items: Query<
        (Entity, Ref<NetId>, ItemQuery, &RootPosition),
        (ValidItemPosition, Or<(Changed<NetId>, ChangedItemFilter, ChangedPositionFilter)>),
    >,
    net_ids: Query<&NetId>,
    mut removed_items: EventReader<OnDestroyNetEntity>,
) {
    for (entity, net_id, item, position) in &changed_items {
        if net_id.is_changed() || item.is_item_changed() || item.position.is_changed() {
            let parent_id = item.parent()
                .and_then(|e| net_ids.get(e).ok())
                .map(|id| id.id);
            let Some(packet) = item.to_upsert(net_id.id, parent_id) else {
                warn!(
                    "failed to create item packet for {entity} (id={:?}, parent_id={:?})",
                    net_id.id,
                    parent_id,
                );
                continue;
            };

            let parent = item.position.parent.map(|p| p.get());
            let packet = Arc::new(packet);
            let grid_cell = delta_grid_cell(position.position.truncate());
            let delta = delta_version.new_delta(DeltaEntry::ItemChanged { entity, parent, packet });

            let mut position_entry = cache.last_position.entry(entity);
            if let Entry::Occupied(entry) = &mut position_entry {
                let last_position = entry.get();
                let last_grid_cell = delta_grid_cell(last_position.position.truncate());

                if last_position.map_id != position.map_id || grid_cell != last_grid_cell {
                    if let Some(cell) = delta_grid.cell_at_mut(last_position.map_id, last_position.position.truncate()) {
                        cell.deltas.push(delta.clone());
                    }
                }
            }

            if let Some(cell) = delta_grid.cell_at_mut(position.map_id, grid_cell) {
                cell.deltas.push(delta);
            }

            position_entry.insert(**position);
        }

        if net_id.is_changed() || item.tooltip.is_changed() {
            let grid_cell = delta_grid_cell(position.position.truncate());
            let packet = EntityTooltipVersion {
                id: net_id.id,
                revision: item.tooltip.version,
            }.into_any_arc();
            let delta = delta_version.new_delta(DeltaEntry::TooltipChanged { entity, packet });
            if let Some(cell) = delta_grid.cell_at_mut(position.map_id, grid_cell) {
                cell.deltas.push(delta);
            }
        }
    }

    for event in removed_items.read() {
        let OnDestroyNetEntity { entity, id } = event.clone();
        if let Some(last_position) = cache.last_position.remove(&entity) {
            let grid_cell = delta_grid_cell(last_position.position.truncate());
            let packet = DeleteEntity {
                id,
            }.into_any_arc();
            let delta = delta_version.new_delta(DeltaEntry::ItemRemoved { entity, packet });

            if let Some(cell) = delta_grid.cell_at_mut(last_position.map_id, grid_cell) {
                cell.deltas.push(delta);
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<ItemQuantity>()
        .register_type::<ItemGraphic>()
        .register_type::<Container>()
        .add_event::<OnContainerOpen>()
        .add_systems(PostUpdate, (
            (
                set_static_root_positions,
                update_root_position,
            ).in_set(ServerSet::UpdateVisibility),
        ))
        .add_systems(Last, (
            detect_item_changes.in_set(ServerSet::DetectChanges),
        ));
}
