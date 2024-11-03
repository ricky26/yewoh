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
pub struct CharacterListEvent {
    pub client_entity: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct CreateCharacterEvent {
    pub client_entity: Entity,
    pub request: CreateCharacter,
}

#[derive(Debug, Clone, Event)]
pub struct SelectCharacterEvent {
    pub client_entity: Entity,
    pub request: SelectCharacter,
}

#[derive(Debug, Clone, Event)]
pub struct DeleteCharacterEvent {
    pub client_entity: Entity,
    pub request: DeleteCharacter,
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<SentCharacterList>()
        .register_type::<User>()
        .add_event::<CharacterListEvent>()
        .add_event::<CreateCharacterEvent>()
        .add_event::<SelectCharacterEvent>()
        .add_event::<DeleteCharacterEvent>();
}