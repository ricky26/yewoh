use crate::data::prefab::{FromPrefabTemplate, PrefabBundle};
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use serde::Deserialize;
use yewoh_server::world::entity::Location;

#[derive(Deserialize)]
pub struct LocationPrefab {
    #[serde(flatten)]
    pub location: Location,
}

impl FromPrefabTemplate for LocationPrefab {
    type Template = LocationPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for LocationPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        world.entity_mut(entity).insert(self.location);
    }
}
