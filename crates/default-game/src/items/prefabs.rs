use std::collections::HashMap;

use bevy::prelude::*;
use bevy::reflect::std_traits::ReflectDefault;
use bevy::reflect::ReflectDeserialize;
use serde::{Deserialize, Serialize};
use bevy_fabricator::Fabricated;
use bevy_fabricator::traits::{Apply, ReflectApply};
use yewoh_server::world::entity::{Container, Flags, Graphic, Hue, Tooltip, TooltipLine};

#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
#[reflect(Default, Serialize, Deserialize, Apply)]
pub struct ItemPrefab {
    graphic: u16,
    #[serde(default)]
    hue: u16,
}

impl Apply for ItemPrefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        world.entity_mut(entity)
            .insert((
                Graphic(self.graphic),
                Hue(self.hue),
                Flags::default(),
            ));
        Ok(())
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
#[reflect(Apply, Deserialize)]
pub struct TooltipPrefab {
    #[serde(flatten)]
    pub entries: HashMap<String, TooltipLinePrefab>,
}

impl Apply for TooltipPrefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
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

        Ok(())
    }
}

#[derive(Clone, Default, Reflect, Deserialize)]
#[reflect(Default, Deserialize, Apply)]
pub struct ContainerPrefab {
    gump: u16,
}

impl Apply for ContainerPrefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        world.entity_mut(entity)
            .insert(Container {
                gump_id: self.gump,
            })
            .insert(Flags::default());
        Ok(())
    }
}
