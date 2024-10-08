use crate::ai::behaviours::wander::{wander, WanderPrefab};
use crate::data::prefab::PrefabAppExt;
use bevy_app::{App, Plugin, Update};

pub mod behaviours;

#[derive(Default)]
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_prefab_bundle::<WanderPrefab>("wander")
            .add_systems(Update, (
                wander,
            ));
    }
}
