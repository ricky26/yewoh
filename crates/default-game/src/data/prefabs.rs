use std::sync::Arc;

use anyhow::anyhow;
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_fabricator::{empty_reflect, FabricateRequest, Fabricated, Fabricator};
use crate::entities::PrefabInstance;

#[derive(Clone, Default, Resource)]
pub struct PrefabLibrary {
    prefabs: HashMap<String, Fabricator>,
}

impl PrefabLibrary {
    pub fn len(&self) -> usize {
        self.prefabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.prefabs.is_empty()
    }

    pub fn get(&self, prefab_name: &str) -> Option<&Fabricator> {
        self.prefabs.get(prefab_name)
    }

    pub fn insert(&mut self, prefab_name: String, fabricator: Fabricator) {
        self.prefabs.insert(prefab_name, fabricator);
    }

    pub fn request_for(&self, request: &PrefabLibraryRequest) -> anyhow::Result<FabricateRequest> {
        let fabricator = self.get(&request.prefab_name)
            .ok_or_else(|| anyhow!("missing prefab {}", &request.prefab_name))?;
        Ok(FabricateRequest {
            factory: fabricator.factory.clone(),
            parameters: request.parameters.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct PrefabLibraryRequest {
    pub prefab_name: String,
    pub parameters: Arc<dyn PartialReflect>,
}

impl PrefabLibraryRequest {
    pub fn with_prefab_name(prefab_name: impl Into<String>) -> PrefabLibraryRequest {
        PrefabLibraryRequest {
            prefab_name: prefab_name.into(),
            parameters: empty_reflect(),
        }
    }
}

impl From<String> for PrefabLibraryRequest {
    fn from(value: String) -> Self {
        PrefabLibraryRequest::with_prefab_name(value)
    }
}

impl From<&String> for PrefabLibraryRequest {
    fn from(value: &String) -> Self {
        PrefabLibraryRequest::with_prefab_name(value)
    }
}

impl From<&str> for PrefabLibraryRequest {
    fn from(value: &str) -> Self {
        PrefabLibraryRequest::with_prefab_name(value)
    }
}

pub fn fabricate_from_library(
    world: &mut World, entity: Entity, request: PrefabLibraryRequest,
) -> anyhow::Result<()> {
    let library = world.resource::<PrefabLibrary>();
    let fabricate_request = library.request_for(&request)?;
    let fabricated = fabricate_request.fabricate(world, entity)?;
    world.entity_mut(entity)
        .insert((
            fabricated,
        ));
    Ok(())
}

fn fabricate_prefab_impl(
    world: &mut World, entity: Entity, prefab_name: &str,
) -> anyhow::Result<Fabricated> {
    let library = world.resource::<PrefabLibrary>();
    let request = PrefabLibraryRequest::from(prefab_name);
    let fabricate_request = library.request_for(&request)?;
    let fabricated = fabricate_request.fabricate(world, entity)?;
    Ok(fabricated)
}

pub fn fabricate_prefab(
    world: &mut World, entity: Entity, prefab_name: &str,
) -> anyhow::Result<()> {
    let prefab_instance = PrefabInstance {
        prefab_name: prefab_name.to_string(),
    };
    match fabricate_prefab_impl(world, entity, prefab_name) {
        Ok(fabricated) => {
            world.entity_mut(entity)
                .insert((
                    fabricated,
                    prefab_instance,
                ));
            Ok(())
        }
        Err(err) => {
            world.entity_mut(entity)
                .insert(prefab_instance);
            Err(err)
        }
    }
}

pub trait PrefabLibraryWorldExt {
    type EntityMut<'a> where Self: 'a;

    fn fabricate_from_library(&mut self, request: impl Into<PrefabLibraryRequest>) -> Self::EntityMut<'_>;

    fn fabricate_prefab(&mut self, prefab_name: impl Into<String>) -> Self::EntityMut<'_>;
}

impl PrefabLibraryWorldExt for World {
    type EntityMut<'a> = EntityWorldMut<'a>;

    fn fabricate_from_library(&mut self, request: impl Into<PrefabLibraryRequest>) -> Self::EntityMut<'_> {
        let mut commands = self.spawn_empty();
        commands.fabricate_from_library(request);
        commands
    }

    fn fabricate_prefab(&mut self, prefab_name: impl Into<String>) -> Self::EntityMut<'_> {
        let mut commands = self.spawn_empty();
        commands.fabricate_prefab(prefab_name);
        commands
    }
}

impl PrefabLibraryWorldExt for Commands<'_, '_> {
    type EntityMut<'a> = EntityCommands<'a> where Self: 'a;

    fn fabricate_from_library(&mut self, request: impl Into<PrefabLibraryRequest>) -> Self::EntityMut<'_> {
        let mut commands = self.spawn_empty();
        commands.fabricate_from_library(request);
        commands
    }

    fn fabricate_prefab(&mut self, prefab_name: impl Into<String>) -> Self::EntityMut<'_> {
        let mut commands = self.spawn_empty();
        commands.fabricate_prefab(prefab_name);
        commands
    }
}

pub trait PrefabLibraryEntityExt {
    fn fabricate_from_library(&mut self, request: impl Into<PrefabLibraryRequest>) -> &mut Self;

    fn fabricate_prefab(&mut self, prefab_name: impl Into<String>) -> &mut Self;
}

impl PrefabLibraryEntityExt for EntityCommands<'_> {
    fn fabricate_from_library(&mut self, request: impl Into<PrefabLibraryRequest>) -> &mut Self {
        let request = request.into();
        self.queue(move |entity, world: &mut World| {
            if let Err(err) = fabricate_from_library(world, entity, request) {
                warn!("failed to fabricate: {err}");
            }
        })
    }

    fn fabricate_prefab(&mut self, prefab_name: impl Into<String>) -> &mut Self {
        let prefab_name = prefab_name.into();
        self.queue(move |entity, world: &mut World| {
            if let Err(err) = fabricate_prefab(world, entity, &prefab_name) {
                warn!("failed to fabricate {prefab_name}: {err}");
            }
        })
    }
}

impl PrefabLibraryEntityExt for EntityWorldMut<'_> {
    fn fabricate_from_library(&mut self, request: impl Into<PrefabLibraryRequest>) -> &mut Self {
        let entity = self.id();
        let request = request.into();
        self.world_scope(move |world| {
            if let Err(err) = fabricate_from_library(world, entity, request) {
                warn!("failed to fabricate: {err}");
            }
        });
        self
    }

    fn fabricate_prefab(&mut self, prefab_name: impl Into<String>) -> &mut Self {
        let entity = self.id();
        let prefab_name = prefab_name.into();
        self.world_scope(move |world| {
            if let Err(err) = fabricate_prefab(world, entity, &prefab_name) {
                warn!("failed to fabricate {prefab_name}: {err}");
            }
        });
        self
    }
}
