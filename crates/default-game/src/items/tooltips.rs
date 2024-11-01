use bevy::prelude::*;
use yewoh_server::world::entity::{TooltipLine, TooltipRequests};

#[derive(Clone, Debug, Reflect, Component)]
#[reflect(Component)]
pub struct StaticTooltips {
    pub entries: Vec<TooltipLine>,
}

pub fn add_static_tooltips(
    mut query: Query<(&mut TooltipRequests, &StaticTooltips), Changed<TooltipRequests>>,
) {
    for (mut requests, tooltips) in &mut query {
        for request in &mut requests.requests {
            request.entries.extend(tooltips.entries.iter().cloned());
        }
    }
}


