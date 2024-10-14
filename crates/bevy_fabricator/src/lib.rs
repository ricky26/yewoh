use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use std::any::TypeId;
use std::sync::Arc;

pub mod document;
mod string;

#[derive(Clone, Reflect, Component)]
#[reflect(from_reflect = false, Component)]
pub struct Fabricable {
    parameters: Vec<(String, TypeId)>,
    #[reflect(ignore)]
    fabricate: Arc<dyn Fn(Entity, &dyn Reflect, &mut Commands) + Send + Sync>,
}

impl FromWorld for Fabricable {
    fn from_world(_world: &mut World) -> Self {
        // TODO: put default fabricate into lazy static.
        Fabricable {
            parameters: Vec::default(),
            fabricate: Arc::new(|_, _, _| {}),
        }
    }
}

#[derive(Clone, Debug, Reflect, Component)]
#[reflect(Component)]
pub struct Fabricate {
    template: Entity,
    parameters: Vec<i32>,
}

impl FromWorld for Fabricate {
    fn from_world(_world: &mut World) -> Self {
        Fabricate {
            template: Entity::PLACEHOLDER,
            parameters: Vec::default(),
        }
    }
}

impl MapEntities for Fabricate {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.template = entity_mapper.map_entity(self.template);
    }
}

#[derive(Clone, Copy, Default, Debug, Reflect, Component)]
#[reflect(Component)]
pub struct Fabricated;

#[derive(Default)]
pub struct FabricatorPlugin;

impl Plugin for FabricatorPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<Fabricable>()
            .register_type::<Fabricate>()
            .register_type::<Fabricated>();
    }
}
