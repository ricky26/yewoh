use bevy::prelude::*;
use yewoh::protocol::TargetType;
use yewoh_server::world::connection::NetClient;
use yewoh_server::world::input::{EntityTargetRequest, EntityTargetResponse};

use crate::{hues, DefaultGameSet};
use crate::data::prefabs::PrefabLibraryEntityExt;
use crate::entities::interactions::OnEntityDoubleClick;
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};
use crate::networking::NetClientExt;

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Default, Component)]
pub struct ButcheringKnife;

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Default, Component)]
pub struct ButcheringPrefab(pub String);

#[derive(Clone, Debug, Reflect, Component)]
#[reflect(Component)]
pub struct ButcheringRequest {
    pub client_entity: Entity,
    pub character: Entity,
    pub butchering_knife: Entity,
}

impl FromWorld for ButcheringRequest {
    fn from_world(_world: &mut World) -> Self {
        ButcheringRequest {
            client_entity: Entity::PLACEHOLDER,
            character: Entity::PLACEHOLDER,
            butchering_knife: Entity::PLACEHOLDER,
        }
    }
}

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Butchered;

pub fn start_butchering(
    mut commands: Commands,
    mut events: EntityEventReader<OnEntityDoubleClick, ButcheringKnife>,
) {
    for event in events.read() {
        commands
            .spawn((
                ButcheringRequest {
                    client_entity: event.client_entity,
                    character: event.character,
                    butchering_knife: event.target,
                },
                EntityTargetRequest {
                    client_entity: event.client_entity,
                    target_type: TargetType::Neutral,
                },
            ));
    }
}

pub fn finish_butchering(
    mut commands: Commands,
    clients: Query<&NetClient>,
    completed_requests: Query<(Entity, &ButcheringRequest, &EntityTargetResponse)>,
    targets: Query<(Entity, &ButcheringPrefab), Without<Butchered>>,
) {
    for (entity, request, response) in &completed_requests {
        commands.entity(entity).despawn_recursive();

        let Some(target) = response.target else {
            continue;
        };

        let Ok((target_entity, prefab)) = targets.get(target) else {
            if let Ok(client) = clients.get(request.client_entity) {
                client.send_system_message_hue("Cannot butcher that", hues::RED);
            }
            continue;
        };

        commands.entity(target_entity)
            .insert(Butchered)
            .fabricate_insert(&prefab.0);
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventRoutePlugin::<OnEntityDoubleClick, ButcheringKnife>::default(),
        ))
        .register_type::<ButcheringKnife>()
        .register_type::<ButcheringPrefab>()
        .register_type::<ButcheringRequest>()
        .register_type::<Butchered>()
        .add_systems(First, (
            (
                start_butchering,
                finish_butchering,
            ).in_set(DefaultGameSet::HandleEvents),
        ));
}
