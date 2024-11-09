use bevy::prelude::*;
use yewoh::Direction;
use yewoh_server::world::characters::CharacterName;
use crate::DefaultGameSet;
use crate::entities::tooltips::{OnRequestEntityTooltip, TooltipLine, TOOLTIP_NAME_PRIORITY};
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};

pub mod player;

pub mod persistence;

pub mod paperdoll;

#[derive(Clone, Debug, Default, Event)]
pub struct OnCharacterMove {
    pub blocked: bool,
    pub direction: Direction,
    pub run: bool,
}

pub fn add_character_name_tooltip(
    names: Query<&CharacterName>,
    mut events: EntityEventReader<OnRequestEntityTooltip, CharacterName>,
) {
    for event in events.read() {
        let Ok(name) = names.get(event.target) else {
            continue;
        };

        event.lines.push(TooltipLine::from_str(name.0.to_string(), TOOLTIP_NAME_PRIORITY));
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventRoutePlugin::<OnRequestEntityTooltip, CharacterName>::default(),
            player::plugin,
            persistence::plugin,
            paperdoll::plugin,
        ))
        .add_event::<OnCharacterMove>()
        .add_systems(First, (
            add_character_name_tooltip.in_set(DefaultGameSet::HandleEvents),
        ));
}
