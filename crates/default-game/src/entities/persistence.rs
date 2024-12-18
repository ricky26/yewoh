use bevy::ecs::entity::MapEntities;
use bevy::ecs::reflect::ReflectMapEntities;
use bevy::ecs::query::WorldQuery;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use yewoh_server::world::entity::{ContainedPosition, EquippedPosition, MapPosition, Hue};

use crate::data::prefabs::PrefabLibraryEntityExt;
use crate::entities::{Persistent, PrefabInstance, UniqueId};
use crate::persistence::{BundleSerializer, SerializationSetupExt};

#[derive(Default)]
pub struct PrefabSerializer;

impl BundleSerializer for PrefabSerializer {
    type Query = &'static PrefabInstance;
    type Filter = With<Persistent>;
    type Bundle = String;

    fn id() -> &'static str {
        "Prefab"
    }

    fn priority() -> i32 {
        -1000
    }

    fn extract(item: &PrefabInstance) -> Self::Bundle {
        item.prefab_name.clone()
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity)
            .fabricate_prefab(bundle)
            .insert(Persistent);
    }
}

#[derive(Default)]
pub struct UniqueIdSerializer;

impl BundleSerializer for UniqueIdSerializer {
    type Query = &'static UniqueId;
    type Filter = With<Persistent>;
    type Bundle = UniqueId;

    fn id() -> &'static str {
        "UniqueId"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        item.clone()
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity).insert(bundle);
    }
}

#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(MapEntities, Serialize, Deserialize)]
pub enum PositionDto {
    Map(MapPosition),
    Equipped { parent: Entity, #[serde(flatten)] position: EquippedPosition },
    Contained { parent: Entity, #[serde(flatten)] position: ContainedPosition },
}

impl MapEntities for PositionDto {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        match self {
            PositionDto::Map(_) => {}
            PositionDto::Equipped { parent, .. } => {
                *parent = entity_mapper.map_entity(*parent);
            }
            PositionDto::Contained { parent, .. } => {
                *parent = entity_mapper.map_entity(*parent);
            }
        }
    }
}

#[derive(Default)]
pub struct PositionSerializer;

impl BundleSerializer for PositionSerializer {
    type Query = (
        Option<&'static MapPosition>,
        Option<&'static Parent>,
        Option<&'static ContainedPosition>,
        Option<&'static EquippedPosition>,
    );
    type Filter = (
        Or<(
            With<MapPosition>,
            (With<Parent>, With<ContainedPosition>),
            (With<Parent>, With<EquippedPosition>),
        )>,
        With<Persistent>,
    );
    type Bundle = PositionDto;

    fn id() -> &'static str {
        "Position"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        let (map_position, parent, container_position, equipped_position) = item;

        if let Some(position) = map_position {
            PositionDto::Map(*position)
        } else {
            let Some(parent) = parent else { unreachable!() };
            let parent = parent.get();

            if let Some(position) = container_position {
                PositionDto::Contained { parent, position: *position }
            } else if let Some(position) = equipped_position {
                PositionDto::Equipped { parent, position: *position }
            } else {
                unreachable!()
            }
        }
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        let mut entity_mut = world.entity_mut(entity);
        match bundle {
            PositionDto::Map(position) => {
                entity_mut.insert(position);
            }
            PositionDto::Equipped { parent, position } => {
                entity_mut.set_parent(parent).insert(position);
            }
            PositionDto::Contained { parent, position } => {
                entity_mut.set_parent(parent).insert(position);
            }
        };
    }
}

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct PersistHue;

#[derive(Default)]
pub struct HueSerializer;

impl BundleSerializer for HueSerializer {
    type Query = &'static Hue;
    type Filter = (With<PersistHue>, With<Persistent>);
    type Bundle = u16;

    fn id() -> &'static str {
        "Hue"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        **item
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity)
            .insert((
                PersistHue,
                Hue(bundle),
            ));
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<PersistHue>()
        .register_serializer::<PrefabSerializer>()
        .register_serializer::<UniqueIdSerializer>()
        .register_serializer::<PositionSerializer>()
        .register_serializer::<HueSerializer>();
}
