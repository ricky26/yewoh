use std::time::{Duration, Instant};

use bevy_ecs::prelude::*;

#[derive(Debug, Clone)]
pub struct TickRate {
    tick_rate: f32,
    interval: Duration,
    deadline: Instant,
}

impl TickRate {
    pub fn tick_rate(&self) -> f32 { self.tick_rate }
    pub fn interval(&self) -> Duration { self.interval }

    pub fn from_rate(tick_rate: f32) -> TickRate {
        let interval = Duration::from_secs_f32(1.0 / tick_rate);
        TickRate {
            tick_rate,
            interval,
            deadline: Instant::now(),
        }
    }

    pub fn from_interval(interval: Duration) -> TickRate {
        let tick_rate = 1.0 / interval.as_secs_f32();
        TickRate {
            tick_rate,
            interval,
            deadline: Instant::now(),
        }
    }
}

impl Default for TickRate {
    fn default() -> Self {
        TickRate::from_rate(50.)
    }
}

pub fn limit_tick_rate(mut tick_rate: ResMut<TickRate>) {
    let now = Instant::now();

    if now < tick_rate.deadline {
        let delta = tick_rate.deadline - now;
        std::thread::sleep(delta);
    }

    let new_deadline = tick_rate.deadline + tick_rate.interval;
    tick_rate.deadline = new_deadline;
}