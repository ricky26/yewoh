use bevy::prelude::*;
use yewoh_server::world::items::ItemQuantity;

use crate::entities::tooltips::{OnRequestEntityTooltip, TooltipLine};
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};
use crate::format::FormatInteger;
use crate::DefaultGameSet;
use crate::l10n::LocalisedString;

#[derive(Clone, Copy, Debug, Default, Deref, DerefMut, Component, Reflect)]
#[reflect(Component)]
pub struct Weight(pub f32);

impl Weight {
    pub fn calculate_stack_weight(weight: f32, quantity: u16) -> u16 {
        (weight * (quantity as f32)).ceil() as u16
    }

    pub fn stack_weight(&self, quantity: u16) -> u16 {
        Self::calculate_stack_weight(**self, quantity)
    }
}

pub fn add_weight_tooltip(
    mut events: EntityEventReader<OnRequestEntityTooltip, Weight>,
    weights: Query<(&Weight, &ItemQuantity)>,
) {
    for event in events.read() {
        let Ok((weight, quantity)) = weights.get(event.target) else {
            continue;
        };

        let weight = weight.stack_weight(**quantity);
        let text_id = if weight == 1 {
            1072788
        } else {
            1072789
        };
        event.lines.push(TooltipLine {
            text: LocalisedString {
                text_id,
                arguments: FormatInteger::from(weight).to_string().into(),
            },
            priority: 1,
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventRoutePlugin::<OnRequestEntityTooltip, Weight>::default(),
        ))
        .register_type::<Weight>()
        .add_systems(First, (
            add_weight_tooltip.in_set(DefaultGameSet::HandleEvents),
        ));
}
