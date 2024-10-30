use std::sync::{Arc, Weak};
use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::{Fabricator, Fabricate, Fabricated};

#[derive(Default, Component, Reflect)]
#[reflect(Component)]
pub struct WatchForFabricatorChanges;

#[derive(Default, Component, Reflect)]
#[reflect(Component)]
pub struct FabricatorChanged;

pub fn mark_changed(
    mut commands: Commands,
    mut events: EventReader<AssetEvent<Fabricator>>,
    asset_server: Res<AssetServer>,
    fabricators: Res<Assets<Fabricator>>,
    updatable_entities: Query<
        (Entity, &Fabricate, &Fabricated),
        (With<WatchForFabricatorChanges>, Without<FabricatorChanged>),
    >,
) {
    if !events.read().any(|e| matches!(e, &AssetEvent::Modified { .. })) {
        return;
    }

    events.clear();
    for (entity, fabricate, fabricated) in &updatable_entities {
        let Some(fabricator) = fabricators.get(&fabricate.fabricator) else {
            if let Some(LoadState::Failed(err)) = asset_server.get_load_state(&fabricate.fabricator) {
                warn!("failed to load fabricator {:?}: {err}", fabricate.fabricator);
            }
            continue;
        };

        let Some(old_factory) = &fabricated.factory else {
            continue;
        };

        let new_factory = Arc::downgrade(&fabricator.factory);
        if Weak::ptr_eq(old_factory, &new_factory) {
            continue;
        }

        commands.entity(entity).insert(FabricatorChanged);
    }
}
