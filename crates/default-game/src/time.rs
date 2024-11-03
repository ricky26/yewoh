use bevy::prelude::*;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use yewoh::protocol::{GlobalLightLevel, Packet, SetTime};
use yewoh_server::world::connection::NetClient;
use yewoh_server::world::view::Synchronizing;

pub const MULTIPLIER: f64 = 12.;

#[derive(Debug, Clone, Copy)]
pub struct WorldTime {
    pub seconds: f64,
}

impl WorldTime {
    pub fn now() -> WorldTime {
        Utc::now().into()
    }

    pub fn day_fraction(&self) -> f32 {
        ((self.seconds / 86400.) % 1.) as f32
    }

    pub fn light_level(&self) -> u8 {
        let float_light_level = 1. + (self.day_fraction() * 2. * std::f32::consts::PI).cos();
        (float_light_level * 6.) as u8
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
        let epoch = DateTime::from_naive_utc_and_offset(
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(1997, 9, 24).unwrap(),
                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            ), real_time.offset().clone());
        let duration = real_time - epoch;
        let seconds = duration.num_milliseconds() as f64 * 0.001 * MULTIPLIER;
        WorldTime { seconds }
    }
}

pub fn send_time(
    new_clients: Query<&NetClient, With<Synchronizing>>,
    all_clients: Query<&NetClient>,
    mut last_light_level: Local<u8>,
) {
    let now = WorldTime::now();
    let (hour, minute, second) = now.hms();
    let light_level = now.light_level();

    if *last_light_level != light_level {
        let packet = SetTime { hour, minute, second }.into_arc();
        let light_level_packet = GlobalLightLevel(light_level).into_arc();
        *last_light_level = light_level;

        for client in all_clients.iter() {
            client.send_packet_arc(packet.clone());
            client.send_packet_arc(light_level_packet.clone());
        }
    } else if !new_clients.is_empty() {
        let packet = SetTime { hour, minute, second }.into_arc();
        let light_level_packet = GlobalLightLevel(light_level).into_arc();

        for client in new_clients.iter() {
            client.send_packet_arc(packet.clone());
            client.send_packet_arc(light_level_packet.clone());
        }
    }
}
