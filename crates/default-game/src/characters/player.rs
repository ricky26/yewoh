use bevy::prelude::*;
use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::entity::{Hue, Stats};

use crate::data::prefabs::PrefabLibraryWorldExt;
use crate::entities::persistence::CustomHue;
use crate::entities::position::PositionExt;

#[derive(Clone, Debug, Reflect, Component)]
#[reflect(Component)]
pub struct NewPlayerCharacter {
    pub shirt_hue: u16,
    pub pants_hue: u16,
}

pub fn spawn_starting_items(
    mut commands: Commands,
    players: Query<(Entity, &NewPlayerCharacter, &Stats)>,
) {
    for (entity, request, stats) in &players {
        commands.entity(entity).remove::<NewPlayerCharacter>();

        commands.fabricate_prefab("backpack")
            .move_to_equipped_position(entity, EquipmentSlot::Backpack);

        commands.fabricate_prefab("test_top")
            .insert((
                CustomHue,
                Hue(request.shirt_hue),
            ))
            .move_to_equipped_position(entity, EquipmentSlot::Top);

        let bottom_name = if stats.female { "test_skirt" } else { "test_pants" };
        commands.fabricate_prefab(bottom_name)
            .insert((
                CustomHue,
                Hue(request.pants_hue),
            ))
            .move_to_equipped_position(entity, EquipmentSlot::Bottom);

        commands.fabricate_prefab("test_shoes")
            .move_to_equipped_position(entity, EquipmentSlot::Shoes);
    }
}
