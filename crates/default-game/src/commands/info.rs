use bevy_ecs::archetype::Archetypes;
use bevy_ecs::component::Components;
use bevy_ecs::prelude::*;
use clap::Parser;

use yewoh::protocol::{MessageKind, TargetType, UnicodeTextMessage};
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse};
use yewoh_server::world::net::NetClient;

use crate::commands::{TextCommand, TextCommandQueue};

#[derive(Parser)]
pub struct Info;

impl TextCommand for Info {
    fn aliases() -> &'static [&'static str] {
        &["info"]
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub struct ShowInfoCommand;

pub fn info(
    archetypes: &Archetypes,
    components: &Components,
    clients: Query<&NetClient>,
    completed: Query<(Entity, &EntityTargetRequest, &EntityTargetResponse), With<ShowInfoCommand>>,
    mut exec: TextCommandQueue<Info>,
    mut commands: Commands,
) {
    for (from, _) in exec.iter() {
        commands.spawn()
            .insert(EntityTargetRequest {
                client_entity: from,
                target_type: TargetType::Neutral,
            })
            .insert(ShowInfoCommand);
    }

    for (entity, request, response) in completed.iter() {
        commands.entity(entity).despawn();

        let client = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        if let Some(target) = response.target {
            client.send_packet(UnicodeTextMessage {
                kind: MessageKind::System,
                text: "Picked Target".to_string(),
                hue: 120,
                font: 3,
                ..Default::default()
            }.into());

            if let Some(archetype) = archetypes.iter().filter(|a| a.entities().contains(&target)).next() {
                for component in archetype.components() {
                    if let Some(info) = components.get_info(component) {
                        client.send_packet(UnicodeTextMessage {
                            kind: MessageKind::System,
                            text: format!("Has Component {}", info.name()),
                            hue: 120,
                            font: 3,
                            ..Default::default()
                        }.into());
                    }
                }
            }
        } else {
            client.send_packet(UnicodeTextMessage {
                kind: MessageKind::System,
                text: "Target does not exist".to_string(),
                hue: 120,
                font: 3,
                ..Default::default()
            }.into());
        }
    }
}
