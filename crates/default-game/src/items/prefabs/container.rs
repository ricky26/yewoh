use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::reflect::Reflect;
use serde::Deserialize;
use bevy_fabricator::Fabricated;
use bevy_fabricator::traits::Apply;
use yewoh_server::world::entity::{Container, Flags};

/*
#[derive(Clone, Default, Reflect, Deserialize)]
pub struct ContainedItemPrefab {
    pub position: IVec2,
    pub grid_index: u8,
    #[serde(flatten)]
    pub prefab: Prefab,
}
 */

#[derive(Clone, Default, Reflect, Deserialize)]
pub struct ContainerPrefab {
    gump: u16,
    // #[serde(default)]
    // contents: Vec<ContainedItemPrefab>,
}

impl Apply for ContainerPrefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        /*
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
         */

        world.entity_mut(entity)
            .insert(Container {
                gump_id: self.gump,
                items: Vec::new(),
            })
            .insert(Flags::default());

        Ok(())
    }
}
