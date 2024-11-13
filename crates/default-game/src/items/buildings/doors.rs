use bevy::prelude::*;
use glam::ivec3;
use yewoh_server::world::entity::{Direction, MapPosition};
use yewoh_server::world::items::ItemGraphic;
use yewoh_server::world::sound::OnSound;
use crate::DefaultGameSet;
use crate::entities::interactions::OnEntityDoubleClick;
use crate::entity_events::{EntityEventReader, EntityEventRoutePlugin};

#[derive(Clone, Default, Debug, Reflect, Component)]
#[reflect(Default, Component)]
#[require(ItemGraphic)]
pub struct Door {
    pub opened: bool,
    pub open_offset: IVec3,
    pub open_graphic: u16,
    pub closed_graphic: u16,
    pub open_sound: u16,
    pub close_sound: u16,
}

impl Door {
    pub fn graphic(&self) -> u16 {
        if self.opened {
            self.open_graphic
        } else {
            self.closed_graphic
        }
    }
}

#[derive(Clone, Default, Debug, Reflect, Component)]
#[reflect(Default, Component)]
#[require(Direction)]
pub struct FourWayDoor {
    pub graphic: u16,
    pub has_offset: bool,
}

#[derive(Clone, Default, Debug, Reflect, Component)]
#[reflect(Default, Component)]
pub struct DoorCcw;

pub fn update_four_way_doors(
    mut four_way_doors: Query<
        (&mut Door, &FourWayDoor, Has<DoorCcw>, &mut MapPosition, &Direction),
        Or<(Changed<FourWayDoor>, Changed<MapPosition>)>,
    >,
) {
    for (mut door, four_way, ccw, mut position, direction) in &mut four_way_doors {
        let (graphic, offset) = match (*direction, ccw) {
            (Direction::North, false) => (3, ivec3(1, -1, 0)),
            (Direction::North, true) => (1, ivec3(1, 1, 0)),
            (Direction::East, false) => (4, ivec3(1, 1, 0)),
            (Direction::East, true) => (6, IVec3::ZERO),
            (Direction::South, false) => (0, ivec3(-1, 1, 0)),
            (Direction::South, true) => (2, IVec3::NEG_X),
            (Direction::West, false) => (7, IVec3::NEG_Y),
            (Direction::West, true) => (5, ivec3(1, -1, 0)),
            _ => continue,
        };
        let offset = if four_way.has_offset { offset } else { IVec3::ZERO };

        let door = door.as_mut();
        door.closed_graphic = four_way.graphic + (graphic << 1);
        door.open_graphic = door.closed_graphic + 1;

        if door.opened && door.open_offset != offset {
            position.position += offset - door.open_offset;
        }

        door.open_offset = offset;
    }
}

pub fn update_door_graphic(
    mut doors: Query<(&mut ItemGraphic, &Door), Changed<Door>>,
) {
    for (mut graphic, door) in &mut doors {
        **graphic = door.graphic();
    }
}

pub fn double_click_doors(
    mut events: EntityEventReader<OnEntityDoubleClick, Door>,
    mut doors: Query<(&mut Door, &mut MapPosition)>,
    mut sounds: EventWriter<OnSound>,
) {
    for event in events.read() {
        let Ok((mut door, mut position)) = doors.get_mut(event.target) else {
            continue;
        };

        let sound_id = if door.opened {
            door.close_sound
        } else {
            door.open_sound
        };
        if sound_id != 0 {
            sounds.send(OnSound {
                sound_id,
                position: *position,
                ..default()
            });
        }

        if door.opened {
            position.position -= door.open_offset;
        }

        door.opened = !door.opened;

        if door.opened {
            position.position += door.open_offset;
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .add_plugins((
            EntityEventRoutePlugin::<OnEntityDoubleClick, Door>::default(),
        ))
        .register_type::<Door>()
        .register_type::<FourWayDoor>()
        .register_type::<DoorCcw>()
        .add_systems(First, (
            double_click_doors.in_set(DefaultGameSet::HandleEvents),
        ))
        .add_systems(Update, (
            (
                update_four_way_doors,
                update_door_graphic,
            ).chain(),
        ));
}
