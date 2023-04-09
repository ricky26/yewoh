use std::sync::Arc;
use bevy_ecs::component::Component;

#[derive(Debug, Clone, Copy, Default, Component)]
pub struct Persistent;

#[derive(Debug, Clone, Component)]
pub struct PrefabInstance {
    pub prefab_name: Arc<str>,
}

