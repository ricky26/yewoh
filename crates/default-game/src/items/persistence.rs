use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{FromWorld, World};
use bevy_ecs::query::{With, WorldQuery};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use yewoh_server::world::entity::{Container, EquippedBy, Graphic, ParentContainer};

use crate::entities::Persistent;
use crate::persistence::{BundleSerializer, DeserializeContext, SerializeContext};

pub struct ItemSerializer;

impl FromWorld for ItemSerializer {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl BundleSerializer for ItemSerializer {
    type Query = (
        &'static Graphic,
        Option<&'static Container>,
        Option<&'static ParentContainer>,
        Option<&'static EquippedBy>,
    );
    type Filter = With<Persistent>;
    type Bundle = Graphic;

    fn id() -> &'static str {
        "Item"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        let (graphic, ..) = item;
        graphic.clone()
    }

    fn serialize<S: Serializer>(_ctx: &SerializeContext, s: S, bundle: &Self::Bundle) -> Result<S::Ok, S::Error> {
        bundle.serialize(s)
    }

    fn deserialize<'de, D: Deserializer<'de>>(ctx: &mut DeserializeContext, d: D, entity: Entity) -> Result<(), D::Error> {
        let bundle = Graphic::deserialize(d)?;
        ctx.world_mut().entity_mut(entity).insert(bundle);
        Ok(())
    }
}
