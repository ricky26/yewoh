use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy::utils::{ConditionalSendFuture, HashMap};

use crate::document::Document;
use crate::{convert, Fabricable};
use crate::parser::FilePosition;
use crate::prefab::DocumentMap;

#[derive(Clone, Reflect, Asset)]
#[reflect(from_reflect = false)]
pub struct Fabricator {
    pub fabricable: Fabricable,
}

pub struct FabricatorLoader {
    type_registry: AppTypeRegistry,
}

impl FabricatorLoader {
    pub fn new(type_registry: AppTypeRegistry) -> FabricatorLoader {
        FabricatorLoader {
            type_registry,
        }
    }
}

impl AssetLoader for FabricatorLoader {
    type Asset = Fabricator;
    type Settings = ();
    type Error = anyhow::Error;

    fn load(&self,
            reader: &mut dyn Reader,
            _settings: &Self::Settings,
            load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output=Result<Self::Asset, Self::Error>> {
        let type_registry = self.type_registry.clone();
        let file_path = load_context.path().to_string_lossy().to_string();

        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;

            let src = String::from_utf8(bytes)?;
            let doc = Document::parse(&src)
                .map_err(|e| e.map_position(|p| FilePosition::from_file_and_str(&file_path, &src, p)))?;

            let dep_paths = doc.dependencies();
            let mut deps = HashMap::new();

            for dep_path in dep_paths {
                let loaded = load_context.loader().immediate().load::<Fabricator>(&dep_path).await?;
                deps.insert(dep_path, loaded.get().fabricable.fabricate.clone());
            }

            let type_registry = type_registry.read();
            let documents = DocumentMap(deps);
            let fabricable = convert(&type_registry, &documents, &doc)?;

            Ok(Fabricator {
                fabricable,
            })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["fab"]
    }
}

#[derive(Component)]
pub struct LoadFabricator(Handle<Fabricator>);

pub fn load_fabricators(
    mut commands: Commands,
    fabricators: Res<Assets<Fabricator>>,
    query: Query<(Entity, &LoadFabricator), Without<Fabricable>>,
) {
    for (entity, request) in &query {
        let Some(fabricator) = fabricators.get(&request.0) else { continue };
        commands.entity(entity).insert(fabricator.fabricable.clone());
    }
}
