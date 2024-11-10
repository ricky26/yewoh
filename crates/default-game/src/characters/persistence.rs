use bevy::ecs::query::WorldQuery;
use bevy::prelude::*;
use yewoh_server::world::characters::{CharacterName, CharacterStats};

use crate::entities::Persistent;
use crate::persistence::{BundleSerializer, SerializationSetupExt};

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct PersistName;

#[derive(Default)]
pub struct NameSerializer;

impl BundleSerializer for NameSerializer {
    type Query = &'static CharacterName;
    type Filter = (With<PersistName>, With<Persistent>);
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
                PersistName,
                CharacterName(bundle),
            ));
    }
}

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct PersistStats;

#[derive(Default)]
pub struct StatsSerializer;

impl BundleSerializer for StatsSerializer {
    type Query = &'static CharacterStats;
    type Filter = (With<PersistStats>, With<Persistent>);
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
                PersistStats,
                bundle,
            ));
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<PersistStats>()
        .register_serializer::<NameSerializer>()
        .register_serializer::<StatsSerializer>();
}
