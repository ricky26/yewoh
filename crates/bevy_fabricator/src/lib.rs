#![allow(clippy::type_complexity)]

use std::any::TypeId;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, LazyLock, Weak};
use anyhow::bail;
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
pub mod glam;

#[cfg(feature = "humantime")]
pub mod humantime;

pub type Factory = Arc<dyn Fn(Entity, &dyn PartialReflect, &mut World) -> anyhow::Result<Fabricated> + Send + Sync>;
pub type WeakFactory = Weak<dyn Fn(Entity, &dyn PartialReflect, &mut World) -> anyhow::Result<Fabricated> + Send + Sync>;

#[derive(Clone, Debug, Reflect)]
pub struct FabricationParameter {
    pub parameter_type: TypeId,
    pub optional: bool,
}

#[derive(Clone, Reflect, Asset)]
#[reflect(opaque, Debug)]
pub struct Fabricator {
    pub parameters: HashMap<String, FabricationParameter>,
    pub factory: Factory,
}

impl Fabricator {
    pub fn fabricate(
        &self, parameters: &dyn PartialReflect, world: &mut World, entity: Entity,
    ) -> anyhow::Result<Fabricated> {
        (self.factory)(entity, parameters, world)
    }
}

impl Debug for Fabricator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Fabricator");

        for (k, v) in &self.parameters {
            s.field(k, v);
        }

        s.finish()
    }
}

static EMPTY_REFLECT: LazyLock<Arc<dyn PartialReflect>> = LazyLock::new(|| Arc::new(()));

pub fn empty_reflect() -> Arc<dyn PartialReflect> {
    EMPTY_REFLECT.clone()
}

#[derive(Clone, Debug, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Fabricate {
    pub fabricator: Handle<Fabricator>,
    pub parameters: Arc<dyn PartialReflect>,
}

impl Fabricate {
    pub fn with_handle(fabricator: Handle<Fabricator>) -> Fabricate {
        Fabricate {
            fabricator,
            parameters: empty_reflect(),
        }
    }

    pub fn to_request(
        &self, fabricators: &Assets<Fabricator>, asset_server: Option<&AssetServer>,
    ) -> anyhow::Result<Option<FabricateRequest>> {
        match fabricators.get(&self.fabricator) {
            Some(template) => Ok(Some(FabricateRequest {
                factory: template.factory.clone(),
                parameters: self.parameters.clone(),
            })),
            None => {
                let result = asset_server
                    .and_then(|s| s.get_load_state(&self.fabricator));
                if let Some(LoadState::Failed(err)) = result {
                    bail!("failed to fabricate: {err}");
                }
                Ok(None)
            }
        }
    }
}

impl Default for Fabricate {
    fn default() -> Self {
        Fabricate {
            fabricator: Handle::default(),
            parameters: empty_reflect(),
        }
    }
}

#[derive(Clone)]
pub struct FabricateRequest {
    pub factory: Factory,
    pub parameters: Arc<dyn PartialReflect>,
}

impl FabricateRequest {
    pub fn fabricate(&self, world: &mut World, entity: Entity) -> anyhow::Result<Fabricated> {
        let mut result = (self.factory)(entity, self.parameters.as_ref(), world)?;
        result.factory = Some(Arc::downgrade(&self.factory));
        Ok(result)
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
        let request = match request.to_request(&templates, Some(&asset_server)) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(err) => {
                commands.entity(entity).insert(Fabricated::default());
                warn!("{err}");
                continue;
            },
        };

        commands.queue(move |world: &mut World| {
            match request.fabricate(world, entity) {
                Ok(result) => {
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
        request: impl Into<FabricateRequest>,
    ) -> &mut Self;
}

impl FabricateExt for EntityCommands<'_> {
    fn fabricate(
        &mut self,
        request: impl Into<FabricateRequest>,
    ) -> &mut Self {
        let request = request.into();
        self.queue::<()>(move |entity, world: &mut World| {
            if let Err(err) = request.fabricate(world, entity) {
                error!("failed to fabricate: {err}");
            }
        });
        self
    }
}

impl FabricateExt for EntityWorldMut<'_> {
    fn fabricate(
        &mut self,
        request: impl Into<FabricateRequest>,
    ) -> &mut Self {
        let request = request.into();
        let entity = self.id();
        self.world_scope(|world| {
            match request.fabricate(world, entity) {
                Ok(fabricated) => {
                    world.entity_mut(entity).insert(fabricated);
                }
                Err(err) => {
                    error!("failed to fabricate: {err}");
                }
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
            .add_plugins((
                glam::register,
            ))
            .register_type::<Fabricate>()
            .register_type::<Fabricated>()
            .register_type::<any::Any>()
            .register_type::<values::Some>()
            .register_type::<values::None>()
            .register_type::<operations::Spawn>()
            .register_type::<operations::Fabricate>()
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
