use std::fmt::Formatter;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use serde::{Deserialize, Deserializer};
use serde::de::{Error, SeqAccess, Visitor};
use crate::data::prefab::{FromPrefabTemplate, PrefabBundle, PrefabCollection};

pub struct InheritancePrefab {
    pub prefabs: Vec<String>,
}

struct InheritancePrefabVisitor;

impl<'de> Visitor<'de> for InheritancePrefabVisitor {
    type Value = InheritancePrefab;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "an inheritance list")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> where E: Error {
        Ok(InheritancePrefab {
            prefabs: vec![v.into()],
        })
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E> where E: Error {
        Ok(InheritancePrefab {
            prefabs: vec![v],
        })
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let mut prefabs = Vec::new();

        if let Some(size) = seq.size_hint() {
            prefabs.reserve(size);
        }

        while let Some(v) = seq.next_element()? {
            prefabs.push(v);
        }

        Ok(InheritancePrefab {
            prefabs,
        })
    }
}

impl<'de> Deserialize<'de> for InheritancePrefab {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_any(InheritancePrefabVisitor)
    }
}

impl FromPrefabTemplate for InheritancePrefab {
    type Template = InheritancePrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for InheritancePrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        let prefabs = world.resource::<PrefabCollection>();
        let prefabs = self.prefabs.iter()
            .filter_map(|name| prefabs.get(name).cloned())
            .collect::<Vec<_>>();
        for prefab in prefabs {
            prefab.write(world, entity);
        }
    }
}
