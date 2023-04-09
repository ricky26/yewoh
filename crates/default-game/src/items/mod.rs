use bevy_app::{App, Plugin};
use crate::data::prefab::PrefabAppExt;
use crate::items::persistence::ItemSerializer;
use crate::items::prefabs::container::ContainerPrefab;
use crate::items::prefabs::ItemPrefab;
use crate::persistence::SerializationSetupExt;

pub mod prefabs;

pub mod persistence;

#[derive(Default)]
pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_prefab_bundle::<ItemPrefab>("item")
            .init_prefab_bundle::<ContainerPrefab>("container")
            .register_serializer::<ItemSerializer>();
    }
}
