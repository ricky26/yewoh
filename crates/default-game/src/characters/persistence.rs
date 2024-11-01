use bevy::ecs::query::WorldQuery;
use bevy::prelude::*;
use yewoh_server::world::entity::Stats;
use crate::entities::Persistent;
use crate::persistence::BundleSerializer;

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct CustomStats;

#[derive(Default)]
pub struct CustomStatsSerializer;

impl BundleSerializer for CustomStatsSerializer {
    type Query = &'static Stats;
    type Filter = (With<CustomStats>, With<Persistent>);
    type Bundle = Stats;

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
