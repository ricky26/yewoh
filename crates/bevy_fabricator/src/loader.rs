use std::path::PathBuf;

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy::utils::{ConditionalSendFuture, HashMap};

use crate::document::Document;
use crate::parser::FilePosition;
use crate::prefab::{convert, FabricatorMap};
use crate::Fabricator;


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
        let file_path = load_context.path().to_path_buf();

        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;

            let file_path_str = file_path.to_string_lossy();
            let src = String::from_utf8(bytes)?;
            let doc = Document::parse(&src)
                .map_err(|e| e.map_position(|p|
                    FilePosition::from_file_and_str(file_path_str.as_ref(), &src, p)))?;

            let dep_paths = doc.dependencies();
            let mut deps = HashMap::new();

            let file_dir = file_path.parent();
            for dep_path in dep_paths {
                let abs_dep_path = file_dir
                    .map_or_else(|| PathBuf::from(&dep_path), |d| d.join(&dep_path));

                let loaded = load_context.loader().immediate().load::<Fabricator>(abs_dep_path.as_path()).await?;
                deps.insert(dep_path, loaded.get().clone());
            }

            let type_registry = type_registry.read();
            let documents = FabricatorMap(deps);
            let fabricator = convert(&type_registry, &documents, &doc)?;
            Ok(fabricator)
        }
    }

    fn extensions(&self) -> &[&str] {
        &["fab"]
    }
}
