use bevy::prelude::*;
use bevy_fabricator::traits::{Apply, ReflectApply};
use bevy_fabricator::Fabricated;
use serde::Deserialize;
use yewoh::Notoriety;
use yewoh_server::world::entity::{BodyType, Hue, MapPosition, Notorious, Stats};

use crate::activities::CurrentActivity;
use crate::characters::{Alive, Animation, HitAnimation};

#[derive(Default, Clone, Reflect, Deserialize)]
#[reflect(Default, Apply, Deserialize)]
#[serde(default)]
pub struct CharacterPrefab {
    pub name: String,
    pub body_type: u16,
    pub hue: u16,
    #[reflect(remote = yewoh_server::remote_reflect::NotorietyRemote)]
    pub notoriety: Notoriety,
    pub hit_animation: Option<Animation>,
}

impl Apply for CharacterPrefab {
    fn apply(
        &self, world: &mut World, entity: Entity, _fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        let mut commands = world.entity_mut(entity);
        commands
            .insert((
                MapPosition::default(),
                Notorious(self.notoriety),
                BodyType(self.body_type),
                Hue(self.hue),
                Stats {
                    name: self.name.to_string(),
                    hp: 100,
                    max_hp: 100,
                    ..Default::default()
                },
                CurrentActivity::Idle,
                Alive,
            ));

        if let Some(hit_animation) = self.hit_animation.clone() {
            commands.insert(HitAnimation { hit_animation });
        }

        Ok(())
    }
}
