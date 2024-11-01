use bevy::ecs::query::WorldQuery;
use bevy::prelude::*;
use yewoh_server::world::entity::Graphic;

use crate::entities::Persistent;
use crate::persistence::BundleSerializer;

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct CustomGraphic;

#[derive(Default)]
pub struct CustomGraphicSerializer;

impl BundleSerializer for CustomGraphicSerializer {
    type Query = &'static Graphic;
    type Filter = (With<CustomGraphic>, With<Persistent>);
    type Bundle = u16;

    fn id() -> &'static str {
        "Graphic"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        **item
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity)
            .insert((
                CustomGraphic,
                Graphic(bundle),
            ));
    }
}
