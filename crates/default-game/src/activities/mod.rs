use bevy_app::{App, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::system::{Query, Res};
use bevy_time::{Time, Timer};

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
            .add_plugin(CombatPlugin)
            .add_systems((
                progress_current_activity,
            ));
    }
}
