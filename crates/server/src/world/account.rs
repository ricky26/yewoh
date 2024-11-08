use bevy::prelude::*;
use yewoh::protocol::{CreateCharacter, DeleteCharacter, SelectCharacter};

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct SentCharacterList;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct User {
    pub username: String,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientCharacterListRequest {
    pub client_entity: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientCreateCharacter {
    pub client_entity: Entity,
    pub request: CreateCharacter,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientSelectCharacter {
    pub client_entity: Entity,
    pub request: SelectCharacter,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientDeleteCharacter {
    pub client_entity: Entity,
    pub request: DeleteCharacter,
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<SentCharacterList>()
        .register_type::<User>()
        .add_event::<OnClientCharacterListRequest>()
        .add_event::<OnClientCreateCharacter>()
        .add_event::<OnClientSelectCharacter>()
        .add_event::<OnClientDeleteCharacter>();
}
