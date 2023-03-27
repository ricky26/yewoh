use bevy_ecs::prelude::*;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use yewoh::protocol::{Packet, SetTime};

use yewoh_server::world::net::{NetClient, NetSynchronizing};

pub const MULTIPLIER: i32 = 12;

#[derive(Debug, Clone, Copy)]
pub struct WorldTime {
    pub seconds: f64,
}

impl WorldTime {
    pub fn now() -> WorldTime {
        Utc::now().into()
    }

    pub fn hms(&self) -> (u8, u8, u8) {
        let total_seconds = self.seconds as i64;
        let seconds = total_seconds % 60;
        let total_minutes = total_seconds / 60;
        let minutes = total_minutes % 60;
        let total_hours = total_minutes / 60;
        let hours = total_hours % 24;
        (hours as u8, minutes as u8, seconds as u8)
    }
}

impl<T: TimeZone> From<DateTime<T>> for WorldTime {
    fn from(real_time: DateTime<T>) -> Self {
        let epoch = DateTime::from_utc(
            NaiveDateTime::new(
                NaiveDate::from_ymd(1997, 9, 24),
                NaiveTime::from_hms(0, 0, 0),
            ), real_time.offset().clone());
        let duration = real_time - epoch;
        let seconds = duration.num_milliseconds() as f64 * (MULTIPLIER as f64);
        WorldTime { seconds }
    }
}

pub fn send_time(
    clients: Query<(&NetClient, &NetSynchronizing)>,
) {
    if clients.is_empty() {
        return;
    }

    let now = WorldTime::now();
    let (hour, minute, second) = now.hms();
    let packet = SetTime { hour, minute, second }.into_arc();
    for (client, _) in clients.iter() {
        client.send_packet_arc(packet.clone());
    }
}
