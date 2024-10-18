use std::any::TypeId;
use std::sync::{Arc, LazyLock};

use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy::utils::HashMap;

pub use prefab::convert;

mod parser;
mod string;
mod prefab;
pub mod document;
pub mod traits;
pub mod operations;

pub type Factory = Arc<dyn Fn(Entity, &dyn PartialReflect, &mut World) -> anyhow::Result<()> + Send + Sync>;

#[derive(Clone, Reflect)]
pub struct FabricationParameter {
    pub parameter_type: TypeId,
    pub optional: bool,
}

#[derive(Clone, Reflect, Component)]
#[reflect(from_reflect = false, Component)]
pub struct Fabricable {
    parameters: HashMap<String, FabricationParameter>,
    #[reflect(ignore)]
    fabricate: Factory,
}

static EMPTY_FABRICATE: LazyLock<Arc<dyn Fn(Entity, &dyn PartialReflect, &mut World) -> anyhow::Result<()> + Send + Sync>> =
    LazyLock::new(|| Arc::new(|_, _, _| Ok(())));
static EMPTY_REFLECT: LazyLock<Arc<dyn PartialReflect>> = LazyLock::new(|| Arc::new(()));

impl FromWorld for Fabricable {
    fn from_world(_world: &mut World) -> Self {
        Fabricable {
            parameters: HashMap::default(),
            fabricate: EMPTY_FABRICATE.clone(),
        }
    }
}

#[derive(Clone, Debug, Reflect, Component)]
#[reflect(Component)]
pub struct Fabricate {
    pub template: Entity,
    pub parameters: Arc<dyn PartialReflect>,
}

impl FromWorld for Fabricate {
    fn from_world(_world: &mut World) -> Self {
        Fabricate {
            template: Entity::PLACEHOLDER,
            parameters: EMPTY_REFLECT.clone(),
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

pub fn fabricate(
    mut commands: Commands,
    to_fabricate: Query<(Entity, &Fabricate), Without<Fabricated>>,
    templates: Query<&Fabricable>,
) {
    for (entity, request) in &to_fabricate {
        let Ok(template) = templates.get(request.template) else { continue };
        let fabricate = template.fabricate.clone();
        let parameters = request.parameters.clone();
        commands.queue(move |world: &mut World| {
            if let Err(err) = fabricate(entity, &parameters, world) {
                warn!("fabrication failed: {err}");
            }
            world.entity_mut(entity).insert(Fabricated);
        });
    }
}

#[derive(Default)]
pub struct FabricatorPlugin;

impl Plugin for FabricatorPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<Fabricable>()
            .register_type::<Fabricate>()
            .register_type::<Fabricated>()
            .add_systems(Update, (
                fabricate,
            ));
    }
}
