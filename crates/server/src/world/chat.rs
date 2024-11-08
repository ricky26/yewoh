use bevy::prelude::*;
use yewoh::protocol::UnicodeTextMessageRequest;

#[derive(Debug, Clone, Event)]
pub struct OnClientChatMessage {
    pub client_entity: Entity,
    pub request: UnicodeTextMessageRequest,
}

pub fn plugin(app: &mut App) {
    app
        .add_event::<OnClientChatMessage>();
}
