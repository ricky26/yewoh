use bevy::ecs::query::WorldQuery;
use bevy::prelude::*;
use yewoh_server::world::items::{ItemGraphic, ItemQuantity};

use crate::entities::Persistent;
use crate::persistence::{BundleSerializer, SerializationSetupExt};

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct PersistGraphic;

#[derive(Default)]
pub struct GraphicSerializer;

impl BundleSerializer for GraphicSerializer {
    type Query = &'static ItemGraphic;
    type Filter = (With<PersistGraphic>, With<Persistent>);
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
                PersistGraphic,
                ItemGraphic(bundle),
            ));
    }
}

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct PersistQuantity;

#[derive(Default)]
pub struct QuantitySerializer;

impl BundleSerializer for QuantitySerializer {
    type Query = &'static ItemQuantity;
    type Filter = (With<PersistQuantity>, With<Persistent>);
    type Bundle = u16;

    fn id() -> &'static str {
        "Quantity"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        **item
    }

    fn insert(world: &mut World, entity: Entity, bundle: Self::Bundle) {
        world.entity_mut(entity)
            .insert((
                PersistQuantity,
                ItemQuantity(bundle),
            ));
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<PersistGraphic>()
        .register_type::<PersistQuantity>()
        .register_serializer::<GraphicSerializer>()
        .register_serializer::<QuantitySerializer>();
}
