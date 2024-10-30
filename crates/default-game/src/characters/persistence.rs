use bevy::ecs::query::{With, WorldQuery};
use bevy::ecs::world::{FromWorld, World};
use bevy::prelude::Entity;

use yewoh_server::world::entity::{Character, Flags, MapPosition, Stats};

use crate::entities::Persistent;
use crate::persistence::BundleSerializer;

pub struct CharacterSerializer;

impl FromWorld for CharacterSerializer {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl BundleSerializer for CharacterSerializer {
    type Query = (
        &'static Character,
        &'static Flags,
        &'static Stats,
        &'static MapPosition,
    );
    type Filter = With<Persistent>;
    type Bundle = (
        Character,
        Flags,
        Stats,
        MapPosition,
    );

    fn id() -> &'static str {
        "Character"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        let (
            character,
            flags,
            stats,
            location,
        ) = item.clone();
        (
            character.clone(),
            flags.clone(),
            stats.clone(),
            location.clone(),
        )
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity).insert(bundle);
    }
}
