use anyhow::Context;
use bevy::ecs::entity::{VisitEntities, VisitEntitiesMut};
use bevy::ecs::reflect::ReflectMapEntities;
use bevy::prelude::*;
use glam::ivec2;
use rand::{thread_rng, Rng, RngCore};
use bevy_fabricator::traits::{Convert, ReflectConvert};
use yewoh_server::world::entity::ContainedPosition;
use yewoh_server::world::items::ItemQuantity;

use crate::data::prefabs::PrefabLibraryWorldExt;
use crate::entities::Persistent;
use crate::entities::position::PositionExt;
use crate::reflect::{assert_struct_fields, reflect_field, reflect_optional_field};

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Default, Component)]
pub struct LootPrefab(pub String);

#[derive(Clone, Debug, Reflect, Component, VisitEntities, VisitEntitiesMut)]
#[reflect(Component, MapEntities, Convert)]
pub struct LootRoll {
    pub target: Entity,
    #[visit_entities(ignore)]
    pub chance: f32,
    #[visit_entities(ignore)]
    pub min_quantity: u16,
    #[visit_entities(ignore)]
    pub max_quantity: u16,
    #[visit_entities(ignore)]
    pub prefab_name: String,
}

impl Convert for LootRoll {
    fn convert(from: Box<dyn PartialReflect>) -> anyhow::Result<Box<dyn PartialReflect>> {
        let from = from.reflect_ref().as_struct().context("converting LootRoll")?;
        assert_struct_fields(from, &["target", "chance", "min_quantity", "max_quantity", "prefab_name"])?;

        let target = reflect_field(from, "target")?;
        let chance = reflect_optional_field::<f32>(from, "chance")?
            .map_or(1., |v| v / 100.);
        let min_quantity = reflect_optional_field(from, "min_quantity")?
            .unwrap_or(1);
        let max_quantity = reflect_optional_field(from, "max_quantity")?
            .unwrap_or(min_quantity);
        let prefab_name = reflect_field(from, "prefab_name")?;

        let value = LootRoll {
            target,
            chance,
            min_quantity,
            max_quantity,
            prefab_name,
        };
        Ok(Box::new(value))
    }
}

impl LootRoll {
    pub fn roll(&self, commands: &mut Commands, rng: &mut impl RngCore) {
        if rng.gen::<f32>() > self.chance {
            return;
        }

        let position = ContainedPosition {
            position: ivec2(0, 0),
            grid_index: 0,
        };

        let quantity = rng.gen_range(self.min_quantity..=self.max_quantity);
        if quantity == 1 {
            commands
                .fabricate_prefab(&self.prefab_name)
                .insert((
                    Persistent,
                ))
                .move_to_container_position(self.target, position);
        } else if quantity > 0 {
            commands
                .fabricate_prefab(&self.prefab_name)
                .insert((
                    Persistent,
                    ItemQuantity(quantity),
                ))
                .move_to_container_position(self.target, position);
        }
    }
}

pub fn spawn_loot(
    mut commands: Commands,
    rolls: Query<(Entity, &LootRoll)>,
) {
    let mut rng = thread_rng();

    for (entity, roll) in &rolls {
        commands.entity(entity).despawn_recursive();

        roll.roll(&mut commands, &mut rng);
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<LootPrefab>()
        .register_type::<LootRoll>()
        .add_systems(Update, (
            spawn_loot,
        ));
}
