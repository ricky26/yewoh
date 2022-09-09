use bevy_app::prelude::*;

use crate::space::{Space, update_space};

pub mod space;

#[derive(Default)]
pub struct DefaultGamePlugin;

impl Plugin for DefaultGamePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<Space>()
            .add_system_to_stage(CoreStage::Last, update_space);
    }

    fn name(&self) -> &str { "Yewoh Default Game" }
}
