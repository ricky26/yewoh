use bevy::prelude::*;

use crate::persistence::SerializationSetupExt;

pub mod tooltips;

pub mod persistence;

#[derive(Default)]
pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<tooltips::StaticTooltips>()
            .register_type::<persistence::CustomGraphic>()
            .register_serializer::<persistence::CustomGraphicSerializer>()
            .add_systems(Update, (
                tooltips::add_static_tooltips,
            ));
    }
}
