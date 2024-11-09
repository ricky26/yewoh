use bevy::prelude::*;
use clap::Parser;
use yewoh::protocol::TargetType;
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse};

use crate::commands::{TextCommand, TextCommandQueue, TextCommandRegistrationExt};

#[derive(Parser, Resource)]
pub struct Destroy;

impl TextCommand for Destroy {
    fn aliases() -> &'static [&'static str] {
        &["destroy", "remove"]
    }
}

#[derive(Debug, Clone, Component)]
pub struct DestroyRequest;

pub fn start_destroy(
    mut exec: TextCommandQueue<Destroy>,
    mut commands: Commands,
) {
    for (from, _) in exec.iter() {
        commands
            .spawn((
                DestroyRequest,
                EntityTargetRequest {
                    client_entity: from,
                    target_type: TargetType::Neutral,
                },
            ));
    }
}

pub fn destroy(
    completed_entity: Query<(Entity, &EntityTargetResponse), With<DestroyRequest>>,
    mut commands: Commands,
) {
    for (entity, response) in completed_entity.iter() {
        commands.entity(entity).despawn();

        let target = match response.target {
            Some(x) => x,
            None => continue,
        };

        commands.entity(target)
            .despawn_recursive();
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_text_command::<Destroy>()
        .add_systems(Update, (
            start_destroy,
            destroy,
        ));
}
