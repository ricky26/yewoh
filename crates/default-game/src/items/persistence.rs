use bevy::ecs::query::{With, WorldQuery};
use bevy::ecs::world::{FromWorld, World};
use bevy::prelude::Entity;
use yewoh_server::world::entity::{Container, ContainerPosition, EquippedPosition, Flags, Graphic, MapPosition};

use crate::entities::Persistent;
use crate::persistence::BundleSerializer;

pub struct ItemSerializer;

impl FromWorld for ItemSerializer {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl BundleSerializer for ItemSerializer {
    type Query = (
        &'static Graphic,
        &'static Flags,
        Option<&'static MapPosition>,
        Option<&'static Container>,
        Option<&'static ContainerPosition>,
        Option<&'static EquippedPosition>,
    );
    type Filter = With<Persistent>;
    type Bundle = (
        Graphic,
        Flags,
        Option<MapPosition>,
        Option<Container>,
        Option<ContainerPosition>,
        Option<EquippedPosition>,
    );

    fn id() -> &'static str {
        "Item"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        let (
            graphic,
            flags,
            location,
            container,
            parent_container,
            equipped_by,
        ) = item.clone();
        (
            graphic.clone(),
            flags.clone(),
            location.cloned(),
            container.cloned(),
            parent_container.cloned(),
            equipped_by.cloned(),
        )
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        let (
            graphic,
            flags,
            location,
            container,
            parent_container,
            equipped_by,
        ) = bundle;

        let mut entity_mut = world.entity_mut(entity);
        entity_mut.insert((graphic, flags));

        if let Some(location) = location {
            entity_mut.insert(location);
        }

        if let Some(container) = container {
            entity_mut.insert(container);
        }

        if let Some(parent_container) = parent_container {
            entity_mut.insert(parent_container);
        }

        if let Some(equipped_by) = equipped_by {
            entity_mut.insert(equipped_by);
        }
    }
}
