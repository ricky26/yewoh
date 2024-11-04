use bevy::ecs::query::WorldQuery;
use bevy::prelude::*;
use yewoh_server::world::characters::{CharacterName, CharacterStats};

use crate::entities::Persistent;
use crate::persistence::BundleSerializer;

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct CustomName;

#[derive(Default)]
pub struct CustomNameSerializer;

impl BundleSerializer for CustomNameSerializer {
    type Query = &'static CharacterName;
    type Filter = (With<CustomName>, With<Persistent>);
    type Bundle = String;

    fn id() -> &'static str {
        "Name"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        item.to_string()
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity)
            .insert((
                CustomName,
                CharacterName(bundle),
            ));
    }
}

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct CustomStats;

#[derive(Default)]
pub struct CustomStatsSerializer;

impl BundleSerializer for CustomStatsSerializer {
    type Query = &'static CharacterStats;
    type Filter = (With<CustomStats>, With<Persistent>);
    type Bundle = CharacterStats;

    fn id() -> &'static str {
        "Stats"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        item.clone()
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity)
            .insert((
                CustomStats,
                bundle,
            ));
    }
}
