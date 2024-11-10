use std::borrow::Cow;
use std::cmp::Ordering;

use bevy::prelude::*;
use yewoh::protocol::{EntityTooltip, EntityTooltipLine};
use yewoh_server::world::connection::NetClient;
use yewoh_server::world::entity::{OnClientTooltipRequest, Tooltip};
use yewoh_server::world::net_id::NetId;
use yewoh_server::world::ServerSet;

use crate::DefaultGameSet;
use crate::entity_events::{EntityEvent, EntityEventRoutePlugin, EntityEventPlugin, EntityEventReader};
use crate::l10n::LocalisedString;

pub const TOOLTIP_NAME_PRIORITY: i32 = -1000;

#[derive(Debug, Clone, Default, Eq, PartialEq, Reflect)]
#[reflect(Default)]
pub struct TooltipLine {
    pub text: LocalisedString<'static>,
    pub priority: i32,
}

impl TooltipLine {
    pub fn from_static(text_id: u32, priority: i32) -> TooltipLine {
        Self {
            text: LocalisedString::from_id(text_id),
            priority,
        }
    }

    pub fn from_str(text: impl Into<Cow<'static, str>>, priority: i32) -> TooltipLine {
        Self {
            text: LocalisedString::from_str(text),
            priority,
        }
    }
}

impl PartialOrd for TooltipLine {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TooltipLine {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
            .then_with(|| self.text.cmp(&other.text))
    }
}

#[derive(Clone, Debug, Event)]
pub struct OnRequestEntityTooltip {
    pub client_entity: Entity,
    pub target: Entity,
    pub lines: Vec<TooltipLine>,
}

impl EntityEvent for OnRequestEntityTooltip {
    fn target(&self) -> Entity {
        self.target
    }
}

#[derive(Clone, Debug, Reflect, Component)]
#[reflect(Component)]
#[require(Tooltip)]
pub struct StaticTooltips {
    pub entries: Vec<TooltipLine>,
}

pub fn add_static_tooltips(
    static_tooltips: Query<&StaticTooltips>,
    mut events: EntityEventReader<OnRequestEntityTooltip, StaticTooltips>,
) {
    for event in events.read() {
        let Ok(static_tooltips) = static_tooltips.get(event.target) else {
            continue;
        };

        event.lines.extend(static_tooltips.entries.iter().cloned());
    }
}

pub fn on_client_tooltip_request(
    mut events: EventReader<OnClientTooltipRequest>,
    mut out_events: EventWriter<OnRequestEntityTooltip>,
) {
    for request in events.read() {
        let client_entity = request.client_entity;
        for target in request.targets.iter().copied() {
            out_events.send(OnRequestEntityTooltip {
                client_entity,
                target,
                lines: Vec::new(),
            });
        }
    }
}

pub fn finish_tooltips(
    clients: Query<&NetClient>,
    net_objects: Query<&NetId>,
    mut events: EntityEventReader<OnRequestEntityTooltip, ()>,
) {
    for event in events.read() {
        let Ok(client) = clients.get(event.client_entity) else {
            continue;
        };

        let Ok(net_id) = net_objects.get(event.target) else {
            continue;
        };

        event.lines.sort_by_key(|l| l.priority);
        let entries = event.lines.drain(..)
            .map(|l| EntityTooltipLine {
                text_id: l.text.text_id,
                params: l.text.arguments.clone(),
            })
            .collect();

        client.send_packet(EntityTooltip::Response {
            id: net_id.id,
            entries,
        })
    }
}

#[derive(Clone, Debug)]
pub struct MarkTooltipChanged;

impl EntityCommand for MarkTooltipChanged {
    fn apply(self, entity: Entity, world: &mut World) {
        if let Some(mut tooltip) = world.entity_mut(entity)
            .get_mut::<Tooltip>()
        {
            tooltip.mark_changed()
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventPlugin::<OnRequestEntityTooltip>::default(),
            EntityEventRoutePlugin::<OnRequestEntityTooltip, ()>::default(),
            EntityEventRoutePlugin::<OnRequestEntityTooltip, StaticTooltips>::default(),
        ))
        .register_type::<TooltipLine>()
        .register_type::<StaticTooltips>()
        .register_type_data::<Vec<TooltipLine>, ReflectFromReflect>()
        .add_systems(First, (
            on_client_tooltip_request.in_set(ServerSet::HandlePackets),
            finish_tooltips.in_set(DefaultGameSet::FinishEvents),
            add_static_tooltips.in_set(DefaultGameSet::HandleEvents),
        ));
}
