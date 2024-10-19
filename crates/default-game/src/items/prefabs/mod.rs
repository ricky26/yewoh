use std::collections::HashMap;

use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use yewoh_server::world::entity::{Flags, Graphic, Tooltip, TooltipLine};

use crate::data::prefab::{FromPrefabTemplate, PrefabBundle};

pub mod container;

#[derive(Debug, Clone, Reflect, Serialize, Deserialize)]
pub struct ItemPrefab {
    graphic: u16,
    #[serde(default)]
    hue: u16,
}

impl FromPrefabTemplate for ItemPrefab {
    type Template = ItemPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for ItemPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        world.entity_mut(entity)
            .insert(Graphic {
                id: self.graphic,
                hue: self.hue,
            })
            .insert(Flags::default());
    }
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TooltipConstructor {
    Localised { text_id: u32, #[serde(default)] arguments: String },
    Literal { text: String },
}

impl Default for TooltipConstructor {
    fn default() -> Self {
        TooltipConstructor::Literal { text: Default::default() }
    }
}

#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
pub struct TooltipLinePrefab {
    #[serde(flatten)]
    pub constructor: TooltipConstructor,
    #[serde(default)]
    pub priority: u32,
}

#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
pub struct TooltipPrefab {
    #[serde(flatten)]
    pub entries: HashMap<String, TooltipLinePrefab>,
}

impl FromPrefabTemplate for TooltipPrefab {
    type Template = TooltipPrefab;

    fn from_template(template: Self::Template) -> Self {
        template
    }
}

impl PrefabBundle for TooltipPrefab {
    fn write(&self, world: &mut World, entity: Entity) {
        let mut tooltip = Tooltip::default();

        for (key, prefab) in &self.entries {
            let priority = prefab.priority;
            let line = match &prefab.constructor {
                TooltipConstructor::Localised { text_id, arguments } => TooltipLine {
                    text_id: *text_id,
                    arguments: arguments.clone(),
                    priority,
                },
                TooltipConstructor::Literal { text } =>
                    TooltipLine::from_str(text.clone(), priority),
            };

            tooltip.push(key, line);
        }

        world.entity_mut(entity)
            .insert(tooltip);
    }
}
