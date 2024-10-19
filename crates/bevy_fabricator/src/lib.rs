use std::any::TypeId;
use std::sync::{Arc, LazyLock};

use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy::utils::HashMap;

use crate::loader::{load_fabricators, Fabricator, FabricatorLoader};

pub use prefab::convert;

mod parser;
mod string;
mod prefab;
pub mod document;
pub mod traits;
pub mod operations;
pub mod loader;

#[cfg(feature = "humantime")]
pub mod humantime;

pub type Factory = Arc<dyn Fn(Entity, &dyn PartialReflect, &mut World) -> anyhow::Result<()> + Send + Sync>;

#[derive(Clone, Reflect)]
pub struct FabricationParameter {
    pub parameter_type: TypeId,
    pub optional: bool,
}

#[derive(Clone, Reflect, Component)]
#[reflect(from_reflect = false, Component)]
pub struct Fabricable {
    pub parameters: HashMap<String, FabricationParameter>,
    #[reflect(ignore)]
    pub fabricate: Factory,
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

#[derive(Clone, Debug, Reflect, Component)]
#[reflect(Component)]
pub struct FabricateAsset {
    pub template: Handle<Fabricator>,
    pub parameters: Arc<dyn PartialReflect>,
}

#[derive(Clone, Copy, Default, Debug, Reflect, Component)]
#[reflect(Component)]
pub struct Fabricated;

pub fn fabricate_entities(
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

pub fn fabricate_assets(
    mut commands: Commands,
    templates: Res<Assets<Fabricator>>,
    to_fabricate: Query<(Entity, &FabricateAsset), Without<Fabricated>>,
) {
    for (entity, request) in &to_fabricate {
        let Some(template) = templates.get(&request.template) else { continue };
        let fabricate = template.fabricable.fabricate.clone();
        let parameters = request.parameters.clone();
        commands.queue(move |world: &mut World| {
            if let Err(err) = fabricate(entity, &parameters, world) {
                warn!("fabrication failed: {err}");
            }
            world.entity_mut(entity).insert(Fabricated);
        });
    }
}

pub trait FabricateExt {
    fn fabricate(
        &mut self,
        factory: impl Into<Factory>,
        parameters: impl Into<Arc<dyn PartialReflect>>,
    ) -> &mut Self;
}

impl FabricateExt for EntityCommands<'_> {
    fn fabricate(
        &mut self,
        factory: impl Into<Factory>,
        parameters: impl Into<Arc<dyn PartialReflect>>,
    ) -> &mut Self {
        let factory = factory.into();
        let parameters = parameters.into();
        self.queue::<()>(move |entity, world: &mut World| {
            if let Err(err) = factory(entity, &parameters, world) {
                error!("failed to fabricate: {err}");
            }
        });
        self
    }
}


#[derive(Default)]
pub struct FabricatorPlugin;

impl Plugin for FabricatorPlugin {
    fn build(&self, app: &mut App) {
        let type_registry = app.world().resource::<AppTypeRegistry>().clone();

        app
            .register_type::<Fabricable>()
            .register_type::<Fabricate>()
            .register_type::<Fabricated>()
            .init_asset::<Fabricator>()
            .register_asset_loader(FabricatorLoader::new(type_registry))
            .add_systems(Update, (
                load_fabricators,
                fabricate_entities.after(load_fabricators),
                fabricate_assets,
            ));

        #[cfg(feature = "humantime")]
        {
            app.register_type::<humantime::HumanDuration>();
        }
    }
}
