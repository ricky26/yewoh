use std::any::TypeId;
use std::sync::{Arc, LazyLock, Weak};
use bevy::asset::LoadState;
use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy::utils::HashMap;

use crate::loader::FabricatorLoader;

mod string;
pub mod parser;
pub mod prefab;
pub mod document;
pub mod traits;
pub mod operations;
pub mod loader;
pub mod any;
pub mod values;
pub mod hot_reload;

#[cfg(feature = "humantime")]
pub mod humantime;

pub type Factory = Arc<dyn Fn(Entity, &dyn PartialReflect, &mut World) -> anyhow::Result<Fabricated> + Send + Sync>;
pub type WeakFactory = Weak<dyn Fn(Entity, &dyn PartialReflect, &mut World) -> anyhow::Result<Fabricated> + Send + Sync>;

#[derive(Clone, Reflect)]
pub struct FabricationParameter {
    pub parameter_type: TypeId,
    pub optional: bool,
}

#[derive(Clone, Reflect, Asset)]
#[reflect(from_reflect = false)]
pub struct Fabricator {
    pub parameters: HashMap<String, FabricationParameter>,
    #[reflect(ignore)]
    pub fabricate: Factory,
}

static EMPTY_REFLECT: LazyLock<Arc<dyn PartialReflect>> = LazyLock::new(|| Arc::new(()));

pub fn empty_reflect() -> Arc<dyn PartialReflect> {
    EMPTY_REFLECT.clone()
}

#[derive(Clone, Debug, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Fabricate {
    pub template: Handle<Fabricator>,
    pub parameters: Arc<dyn PartialReflect>,
}

impl Default for Fabricate {
    fn default() -> Self {
        Fabricate {
            template: Handle::default(),
            parameters: empty_reflect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Reflect, Component)]
#[reflect(Component)]
pub struct FabricatedChild(pub Entity);

impl FromWorld for FabricatedChild {
    fn from_world(_world: &mut World) -> Self {
        FabricatedChild(Entity::PLACEHOLDER)
    }
}

impl MapEntities for FabricatedChild {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.0 = entity_mapper.map_entity(self.0);
    }
}

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct Fabricated {
    #[reflect(ignore)]
    pub factory: Option<WeakFactory>,
    pub children: Vec<Entity>,
}

impl MapEntities for Fabricated {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.children.map_entities(entity_mapper);
    }
}

pub fn fabricate(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    templates: Res<Assets<Fabricator>>,
    to_fabricate: Query<(Entity, &Fabricate), Without<Fabricated>>,
) {
    for (entity, request) in &to_fabricate {
        let Some(template) = templates.get(&request.template) else {
            if let Some(LoadState::Failed(err)) = asset_server.get_load_state(&request.template) {
                commands.entity(entity).insert(Fabricated::default());
                warn!("failed to fabricate {entity:?}: {err}");
            }
            return;
        };
        let fabricate = template.fabricate.clone();
        let parameters = request.parameters.clone();
        commands.queue(move |world: &mut World| {
            match fabricate(entity, &parameters, world) {
                Ok(mut result) => {
                    result.factory = Some(Arc::downgrade(&fabricate));
                    world.entity_mut(entity).insert(result);
                }
                Err(err) => {
                    warn!("fabrication failed: {err}");
                }
            };
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

impl FabricateExt for EntityWorldMut<'_> {
    fn fabricate(
        &mut self,
        factory: impl Into<Factory>,
        parameters: impl Into<Arc<dyn PartialReflect>>,
    ) -> &mut Self {
        let factory = factory.into();
        let parameters = parameters.into();
        let entity = self.id();
        self.world_scope(|world| {
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
            .register_type::<Fabricate>()
            .register_type::<Fabricated>()
            .register_type::<any::Any>()
            .register_type::<values::Some>()
            .register_type::<values::None>()
            .register_type::<hot_reload::WatchForFabricatorChanges>()
            .register_type::<hot_reload::FabricatorChanged>()
            .init_asset::<Fabricator>()
            .register_asset_loader(FabricatorLoader::new(type_registry))
            .add_systems(Update, (
                fabricate,
                hot_reload::mark_changed,
            ));

        #[cfg(feature = "humantime")]
        {
            app.register_type::<humantime::HumanDuration>();
        }
    }
}
