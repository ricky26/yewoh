use std::borrow::Cow;

use bevy::prelude::*;
use yewoh_server::world::items::{ItemGraphic, ItemGraphicOffset, ItemQuantity};

use crate::activities::combat::Corpse;
use crate::DefaultGameSet;
use crate::entities::tooltips::{OnRequestEntityTooltip, TooltipLine, TOOLTIP_NAME_PRIORITY};
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};
use crate::format::FormatInteger;
use crate::l10n::LocalisedString;

#[derive(Clone, Debug, Component, Reflect)]
#[reflect(Component)]
pub enum ItemName {
    Localised(LocalisedString<'static>),
    Dynamic(Cow<'static, str>),
}

pub fn add_item_names(
    mut commands: Commands,
    query: Query<(Entity, &ItemGraphic), Without<ItemName>>,
) {
    for (entity, graphic) in &query {
        commands.entity(entity)
            .insert(ItemName::Localised(
                LocalisedString::from_id(1020000 + (**graphic as u32))));
    }
}

pub fn add_item_name_tooltip(
    names: Query<(&ItemName, &ItemQuantity), Without<Corpse>>,
    mut events: EntityEventReader<OnRequestEntityTooltip, (ItemName, ItemQuantity)>,
) {
    for event in events.read() {
        let Ok((name, quantity)) = names.get(event.target) else {
            continue;
        };

        let text = if **quantity == 1 {
            match name {
                ItemName::Localised(index) => index.clone(),
                ItemName::Dynamic(name) =>
                    LocalisedString::from_str(name.to_string()),
            }
        } else {
            let arguments = match name {
                ItemName::Localised(s) =>
                    format!("{}\t{}", FormatInteger::from(**quantity), s.as_argument()),
                ItemName::Dynamic(name) =>
                    format!("{}\t{}", FormatInteger::from(**quantity), name),
            };
            LocalisedString {
                text_id: 1050039,
                arguments: arguments.into(),
            }
        };
        event.lines.push(TooltipLine {
            text,
            priority: TOOLTIP_NAME_PRIORITY,
        });
    }
}

#[derive(Clone, Debug, Default, Component, Reflect)]
#[reflect(Component)]
pub struct Stackable;

#[derive(Clone, Debug, Default, Reflect)]
pub struct GraphicOffsetEntry {
    pub min_quantity: u16,
    pub offset: u8,
}

#[derive(Clone, Debug, Default, Component, Reflect)]
#[reflect(Component)]
pub struct GraphicOffsetByQuantity(pub Vec<GraphicOffsetEntry>);

pub fn update_graphic_offset_by_quantity(
    mut entities: Query<
        (&mut ItemGraphicOffset, &ItemQuantity, &GraphicOffsetByQuantity),
        Or<(Changed<ItemQuantity>, Changed<GraphicOffsetByQuantity>)>,
    >,
) {
    for (mut offset, quantity, offset_by_quantity) in &mut entities {
        let offset_value = offset_by_quantity.0.iter()
            .position(|entry| entry.min_quantity > **quantity)
            .map_or_else(|| offset_by_quantity.0.last(), |index| {
                if index > 0 {
                    offset_by_quantity.0.get(index - 1)
                } else {
                    None
                }
            })
            .map(|entry| entry.offset)
            .unwrap_or(0);
        offset.0 = offset_value;
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventRoutePlugin::<OnRequestEntityTooltip, (ItemName, ItemQuantity)>::default(),
        ))
        .register_type::<ItemName>()
        .register_type::<Stackable>()
        .register_type::<GraphicOffsetEntry>()
        .register_type::<GraphicOffsetByQuantity>()
        .register_type_data::<Vec<GraphicOffsetEntry>, ReflectFromReflect>()
        .add_systems(First, (
            add_item_name_tooltip.in_set(DefaultGameSet::HandleEvents),
        ))
        .add_systems(Update, (
            update_graphic_offset_by_quantity,
            add_item_names,
        ));
}
