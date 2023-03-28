use std::ops::Deref;

use bevy_ecs::system::Resource;
use tokio::runtime::Handle;

#[derive(Resource, Clone)]
pub struct AsyncRuntime(Handle);

impl Deref for AsyncRuntime {
    type Target = Handle;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Handle> for AsyncRuntime {
    fn from(value: Handle) -> Self {
        Self(value)
    }
}
