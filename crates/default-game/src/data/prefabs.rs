use std::sync::Arc;

use anyhow::anyhow;
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_fabricator::{empty_reflect, FabricateRequest, Fabricator};

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

    pub fn get(&self, id: &str) -> Option<&Fabricator> {
        self.prefabs.get(id)
    }

    pub fn insert(&mut self, id: String, fabricator: Fabricator) {
        self.prefabs.insert(id, fabricator);
    }

    pub fn request_for(&self, request: &PrefabLibraryRequest) -> anyhow::Result<FabricateRequest> {
        let fabricator = self.get(&request.id)
            .ok_or_else(|| anyhow!("missing prefab {}", &request.id))?;
        Ok(FabricateRequest {
            factory: fabricator.factory.clone(),
            parameters: request.parameters.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct PrefabLibraryRequest {
    pub id: String,
    pub parameters: Arc<dyn PartialReflect>,
}

impl PrefabLibraryRequest {
    pub fn with_id(id: impl Into<String>) -> PrefabLibraryRequest {
        PrefabLibraryRequest {
            id: id.into(),
            parameters: empty_reflect(),
        }
    }
}

impl From<String> for PrefabLibraryRequest {
    fn from(value: String) -> Self {
        PrefabLibraryRequest::with_id(value)
    }
}

impl From<&String> for PrefabLibraryRequest {
    fn from(value: &String) -> Self {
        PrefabLibraryRequest::with_id(value)
    }
}

impl From<&str> for PrefabLibraryRequest {
    fn from(value: &str) -> Self {
        PrefabLibraryRequest::with_id(value)
    }
}

pub fn fabricate_from_library(
    world: &mut World, entity: Entity, request: PrefabLibraryRequest,
) -> anyhow::Result<()> {
    let library = world.resource::<PrefabLibrary>();
    let request = library.request_for(&request)?;
    request.fabricate(world, entity)?;
    Ok(())
}

pub trait PrefabLibraryWorldExt {
    type EntityMut<'a> where Self: 'a;

    fn fabricate_from_library(&mut self, request: impl Into<PrefabLibraryRequest>) -> Self::EntityMut<'_>;
}

impl PrefabLibraryWorldExt for World {
    type EntityMut<'a> = EntityWorldMut<'a>;

    fn fabricate_from_library(&mut self, request: impl Into<PrefabLibraryRequest>) -> Self::EntityMut<'_> {
        let mut commands = self.spawn_empty();
        commands.fabricate_from_library(request);
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
}

pub trait PrefabLibraryEntityExt {
    fn fabricate_from_library(&mut self, request: impl Into<PrefabLibraryRequest>) -> &mut Self;
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
}
