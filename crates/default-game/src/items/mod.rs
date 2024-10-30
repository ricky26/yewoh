use bevy::app::{App, Plugin};
use crate::items::persistence::ItemSerializer;
use crate::persistence::SerializationSetupExt;

pub mod prefabs;

pub mod persistence;

#[derive(Default)]
pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_serializer::<ItemSerializer>();
    }
}
