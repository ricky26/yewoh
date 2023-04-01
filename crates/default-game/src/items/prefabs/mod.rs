use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use serde_derive::Deserialize;
use yewoh_server::world::entity::{Flags, Graphic};
use crate::data::prefab::{FromPrefabTemplate, Prefab, PrefabBundle};

pub mod container;

#[derive(Deserialize)]
pub struct ItemPrefab {
    graphic: u16,
    hue: u16,
}

impl FromPrefabTemplate for ItemPrefab {
    type Template = ItemPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for ItemPrefab {
    fn write(&self, _prefab: &Prefab, world: &mut World, entity: Entity) {
        world.entity_mut(entity)
            .insert(Graphic {
                id: self.graphic,
                hue: self.hue,
            })
            .insert(Flags::default());
    }
}
