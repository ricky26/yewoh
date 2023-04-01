use bevy_app::{App, Plugin};
use crate::ai::behaviours::wander::{wander, WanderPrefab};
use crate::data::prefab::PrefabAppExt;

pub mod behaviours;

#[derive(Default)]
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_prefab_bundle::<WanderPrefab>("wander")
            .add_system(wander);
    }
}
