use bevy::prelude::*;
use yewoh::protocol;
use yewoh::protocol::{IntoAnyPacket, PlaySoundEffect};

use crate::world::connection::NetClient;
use crate::world::delta_grid::{delta_grid_cell, DeltaEntry, DeltaGrid, DeltaVersion};
use crate::world::entity::MapPosition;
use crate::world::ServerSet;

#[derive(Debug, Clone, Copy, Default, Reflect)]
#[reflect(Default)]
pub enum SoundKind {
    Ambiance,
    #[default]
    OneShot,
}

impl From<protocol::SoundEffectKind> for SoundKind {
    fn from(value: protocol::SoundEffectKind) -> Self {
        match value {
            protocol::SoundEffectKind::Ambiance => SoundKind::Ambiance,
            protocol::SoundEffectKind::OneShot => SoundKind::OneShot,
        }
    }
}

impl From<SoundKind> for protocol::SoundEffectKind {
    fn from(value: SoundKind) -> Self {
        match value {
            SoundKind::Ambiance => protocol::SoundEffectKind::Ambiance,
            SoundKind::OneShot => protocol::SoundEffectKind::OneShot,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Reflect, Event)]
#[reflect(Default)]
pub struct OnSound {
    pub kind: SoundKind,
    pub sound_id: u16,
    pub position: MapPosition,
}

#[derive(Debug, Clone, Copy, Reflect, Event)]
pub struct OnClientSound {
    pub client_entity: Entity,
    pub kind: SoundKind,
    pub sound_id: u16,
    pub position: IVec3,
}

pub fn queue_sounds(
    delta_version: Res<DeltaVersion>,
    mut delta_grid: ResMut<DeltaGrid>,
    mut events: EventReader<OnSound>,
) {
    for event in events.read() {
        let grid_cell = delta_grid_cell(event.position.position.truncate());
        let packet = PlaySoundEffect {
            kind: event.kind.into(),
            sound_effect_id: event.sound_id,
            position: event.position.position,
        }.into_any_arc();

        if let Some(cell) = delta_grid.cell_at_mut(event.position.map_id, grid_cell) {
            cell.deltas.push(delta_version.new_delta(DeltaEntry::Sound {
                position: event.position.position,
                packet,
            }));
        }
    }
}

pub fn send_sounds(
    mut events: EventReader<OnClientSound>,
    clients: Query<&NetClient>,
) {
    for event in events.read() {
        let Ok(client) = clients.get(event.client_entity) else {
            continue;
        };

        client.send_packet(PlaySoundEffect {
            kind: event.kind.into(),
            sound_effect_id: event.sound_id,
            position: event.position,
        });
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<SoundKind>()
        .register_type::<OnSound>()
        .register_type::<OnClientSound>()
        .add_event::<OnSound>()
        .add_event::<OnClientSound>()
        .add_systems(Last, (
            queue_sounds.in_set(ServerSet::QueueDeltas),
            send_sounds.in_set(ServerSet::Send),
        ));
}
