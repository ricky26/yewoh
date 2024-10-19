use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::reflect::Reflect;
use glam::IVec2;
use serde::Deserialize;

use yewoh_server::world::entity::{Container, Flags, ParentContainer};

use crate::data::prefab::{FromPrefabTemplate, Prefab, PrefabBundle};

#[derive(Clone, Default, Reflect, Deserialize)]
pub struct ContainedItemPrefab {
    pub position: IVec2,
    pub grid_index: u8,
    #[serde(flatten)]
    pub prefab: Prefab,
}

#[derive(Clone, Default, Reflect, Deserialize)]
pub struct ContainerPrefab {
    gump: u16,
    #[serde(default)]
    contents: Vec<ContainedItemPrefab>,
}

impl FromPrefabTemplate for ContainerPrefab {
    type Template = ContainerPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for ContainerPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        let mut items = Vec::with_capacity(self.contents.len());

        for item_template in &self.contents {
            let child_entity = world.spawn_empty()
                .insert(ParentContainer {
                    parent: entity,
                    position: item_template.position,
                    grid_index: item_template.grid_index,
                })
                .id();
            item_template.prefab.write(world, child_entity);
            items.push(child_entity);
        }

        world.entity_mut(entity)
            .insert(Container {
                gump_id: self.gump,
                items,
            })
            .insert(Flags::default());
    }
}
