use bevy::app::{App, Plugin, Update};

use crate::ai::behaviours::wander::{wander, WanderPrefab};

pub mod behaviours;

#[derive(Default)]
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<WanderPrefab>()
            .add_systems(Update, (
                wander,
            ));
    }
}
