use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::system::{Query, Res};
use bevy::time::{Time, Timer};

use crate::activities::combat::CombatPlugin;

pub mod combat;

pub mod loot;

#[derive(Debug, Clone, Component)]
pub enum CurrentActivity {
    Idle,
    Melee(Timer),
}

impl CurrentActivity {
    pub fn is_idle(&self) -> bool {
        matches!(self, CurrentActivity::Idle)
    }
}

pub fn progress_current_activity(time: Res<Time>, mut actors: Query<&mut CurrentActivity>) {
    for mut current_activity in &mut actors {
        if current_activity.is_idle() {
            continue;
        }

        match &mut *current_activity {
            CurrentActivity::Idle => unreachable!(),
            CurrentActivity::Melee(ref mut timer) => {
                if timer.tick(time.delta()).finished() {
                    *current_activity = CurrentActivity::Idle;
                }
            }
        }
    }
}

#[derive(Default)]
pub struct ActivitiesPlugin;

impl Plugin for ActivitiesPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((
                CombatPlugin,
            ))
            .add_systems(Update, (
                progress_current_activity,
            ));
    }
}
