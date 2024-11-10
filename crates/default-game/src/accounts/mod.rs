use bevy::prelude::*;
use glam::IVec3;
use std::collections::HashMap;
use std::marker::PhantomData;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use yewoh::protocol::{CharacterFromList, CharacterList, CharacterListFlags};
use yewoh::types::FixedString;
use yewoh_server::async_runtime::AsyncRuntime;
use yewoh_server::world::account::{OnClientDeleteCharacter, OnClientCharacterListRequest, OnClientCreateCharacter, OnClientSelectCharacter, User};
use yewoh_server::world::characters::{CharacterBodyType, CharacterName, CharacterRace};
use yewoh_server::world::connection::{NetClient, OwningClient, Possessing};
use yewoh_server::world::entity::{Direction, EquipmentSlot, Hue, MapPosition};
use yewoh_server::world::items::ItemGraphic;
use yewoh_server::world::ServerSet;

use crate::accounts::repository::{AccountCharacters, AccountRepository, CharacterToSpawn, NewCharacterInfo};
use crate::characters::persistence::{CustomName, CustomStats};
use crate::characters::player::NewPlayerCharacter;
use crate::data::prefabs::PrefabLibraryWorldExt;
use crate::data::static_data::StaticData;
use crate::entities::persistence::CustomHue;
use crate::entities::position::PositionExt;
use crate::entities::{Persistent, UniqueId};

pub mod repository;

pub mod memory;

pub mod sql;

pub const DEFAULT_CHARACTER_SLOTS: usize = 6;

#[derive(Resource)]
pub struct PendingCharacterLists {
    tx: mpsc::UnboundedSender<(Entity, anyhow::Result<AccountCharacters>)>,
    rx: mpsc::UnboundedReceiver<(Entity, anyhow::Result<AccountCharacters>)>,
}

impl Default for PendingCharacterLists {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { tx, rx }
    }
}

#[derive(Resource)]
pub struct PendingCharacterInfo {
    tx: mpsc::UnboundedSender<(Entity, anyhow::Result<CharacterToSpawn>)>,
    rx: mpsc::UnboundedReceiver<(Entity, anyhow::Result<CharacterToSpawn>)>,
}

impl Default for PendingCharacterInfo {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { tx, rx }
    }
}

pub fn on_list_characters<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    account_repository: Res<T>,
    users: Query<&User>,
    pending: Res<PendingCharacterLists>,
    mut events: EventReader<OnClientCharacterListRequest>,
) {
    for request in events.read() {
        let Ok(user) = users.get(request.client_entity) else {
            continue;
        };

        let username = user.username.clone();
        let tx = pending.tx.clone();
        let entity = request.client_entity;
        let repository = account_repository.clone();
        runtime.spawn(async move {
            tx.send((entity, repository.list_characters(&username).await)).ok();
        });
    }
}

pub fn handle_list_characters_callback(
    clients: Query<&NetClient>,
    static_data: Res<StaticData>,
    mut pending: ResMut<PendingCharacterLists>,
    players_query: Query<(Entity, &UniqueId, &CharacterName), With<CharacterBodyType>>,
    mut all_players: Local<HashMap<Uuid, (Entity, String)>>,
) {
    let mut first = true;

    while let Ok((entity, result)) = pending.rx.try_recv() {
        let Ok(client) = clients.get(entity) else {
            continue;
        };

        if first {
            first = false;
            all_players.clear();
            all_players.extend(players_query.iter()
                .map(|(entity, pc, name)| (pc.id, (entity, name.to_string()))));
        }

        match result {
            Ok(characters) => {
                let mut flags = CharacterListFlags::ALLOW_OVERWRITE_CONFIG
                    | CharacterListFlags::CONTEXT_MENU
                    | CharacterListFlags::PALADIN_NECROMANCER_TOOLTIPS
                    | CharacterListFlags::SAMURAI_NINJA
                    | CharacterListFlags::ELVES
                    | CharacterListFlags::NEW_MOVEMENT_SYSTEM
                    | CharacterListFlags::ALLOW_FELUCCA;

                if characters.len() > 6 {
                    flags |= CharacterListFlags::SEVENTH_CHARACTER_SLOT;
                }

                if characters.len() > 5 {
                    flags |= CharacterListFlags::SIXTH_CHARACTER_SLOT;
                }

                if characters.len() == 1 {
                    flags |= CharacterListFlags::SLOT_LIMIT
                        | CharacterListFlags::SINGLE_CHARACTER_SLOT;
                }

                for (player, id) in &all_players {
                    debug!("existing player id={} e={:?} n={}", player, &id.0, &id.1);
                }

                let character_list = CharacterList {
                    characters: characters.into_iter()
                        .map(|c|
                            c.and_then(|c| all_players.get(&c.id))
                                .map(|(_, name)| CharacterFromList {
                                    name: FixedString::from_str(name),
                                    ..Default::default()
                                }))
                        .collect(),
                    cities: static_data.cities.to_starting_cities().into(),
                    flags,
                };

                client.send_packet(character_list);
            }
            Err(err) => warn!("Failed to list characters: {err}"),
        }
    }
}

pub fn on_create_character<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    repository: Res<T>,
    users: Query<&User>,
    pending: Res<PendingCharacterInfo>,
    mut events: EventReader<OnClientCreateCharacter>,
) {
    for request in events.read() {
        let Ok(user) = users.get(request.client_entity) else {
            continue;
        };

        let repository = repository.clone();
        let entity = request.client_entity;
        let username = user.username.clone();
        let request = request.request.clone();
        let tx = pending.tx.clone();
        runtime.spawn(async move {
            tx.send((entity, repository.create_character(&username, request).await)).ok();
        });
    }
}

pub fn on_select_character<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    repository: Res<T>,
    users: Query<&User>,
    pending: Res<PendingCharacterInfo>,
    mut events: EventReader<OnClientSelectCharacter>,
) {
    for request in events.read() {
        let Ok(user) = users.get(request.client_entity) else {
            continue;
        };

        let repository = repository.clone();
        let username = user.username.clone();
        let tx = pending.tx.clone();
        let character_index = request.request.character_index as i32;
        let entity = request.client_entity;
        runtime.spawn(async move {
            tx.send((entity, repository.load_character(&username, character_index).await)).ok();
        });
    }
}

pub fn on_delete_character<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    repository: Res<T>,
    users: Query<&User>,
    pending: ResMut<PendingCharacterLists>,
    mut events: EventReader<OnClientDeleteCharacter>,
) {
    for request in events.read() {
        let Ok(user) = users.get(request.client_entity) else {
            continue;
        };

        let repository = repository.clone();
        let entity = request.client_entity;
        let username = user.username.clone();
        let request = request.request.clone();
        let tx = pending.tx.clone();
        runtime.spawn(async move {
            if let Err(err) = repository.delete_character(&username, request).await {
                warn!("failed to delete character: {err}");
            }

            tx.send((entity, repository.list_characters(&username).await)).ok();
        });
    }
}

pub fn create_new_character(
    commands: &mut Commands,
    info: NewCharacterInfo,
) -> Entity {
    let race_name = match info.race {
        CharacterRace::Human => "human",
        CharacterRace::Elf => "elf",
        CharacterRace::Gargoyle => "gargoyle",
    };

    let gender_name = match info.female {
        false => "male",
        true => "female",
    };

    let new_character = NewPlayerCharacter {
        shirt_hue: info.shirt_hue,
        pants_hue: info.pants_hue,
    };

    let prefab_name = format!("player_{race_name}_{gender_name}");
    let entity = commands
        .fabricate_prefab(prefab_name)
        .insert((
            Persistent,
            CustomStats,
            CustomName,
            CustomHue,
            CharacterName(info.name.clone()),
            Hue(info.hue),
            info.stats,
            new_character,
            MapPosition {
                map_id: 1,
                position: IVec3::new(1325, 1624, 55),
                direction: Direction::North,
            },
        ))
        .id();

    if info.hair != 0 {
        commands
            .fabricate_prefab("hair")
            .insert((
                Persistent,
                ItemGraphic(info.hair),
                Hue(info.hair_hue),
            ))
            .move_to_equipped_position(entity, EquipmentSlot::Hair);
    }

    if info.beard != 0 {
        commands
            .fabricate_prefab("hair")
            .insert((
                Persistent,
                ItemGraphic(info.beard),
                Hue(info.beard_hue),
            ))
            .move_to_equipped_position(entity, EquipmentSlot::FacialHair);
    }

    entity
}

#[allow(clippy::too_many_arguments)]
pub fn handle_spawn_character<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    mut pending: ResMut<PendingCharacterInfo>,
    pending_list: ResMut<PendingCharacterLists>,
    mut commands: Commands,
    existing_players_query: Query<(Entity, &UniqueId), With<CharacterBodyType>>,
    mut all_players: Local<HashMap<Uuid, Entity>>,
    users: Query<&User>,
    account_repository: Res<T>,
) {
    let mut first = true;

    while let Ok((entity, result)) = pending.rx.try_recv() {
        let client_entity = entity;
        let info = match result {
            Ok(x) => x,
            Err(err) => {
                warn!("While spawning character: {err}");
                continue;
            }
        };

        if first {
            first = false;
            all_players.clear();
            all_players.extend(existing_players_query.iter()
                .map(|(entity, pc)| (pc.id, entity)));
        }

        let primary_entity = match info {
            CharacterToSpawn::ExistingCharacter(id) => {
                info!("Attaching to existing character: {}", &id);
                if let Some(character_entity) = all_players.get(&id).copied() {
                    character_entity
                } else {
                    warn!("Failed to connect to existing character: {}", &id);
                    let user = match users.get(client_entity) {
                        Ok(x) => x,
                        _ => continue,
                    };

                    let username = user.username.clone();
                    let tx = pending_list.tx.clone();
                    let repository = account_repository.clone();
                    runtime.spawn(async move {
                        tx.send((client_entity, repository.list_characters(&username).await)).ok();
                    });
                    continue;
                }
            }
            CharacterToSpawn::NewCharacter(id, info) => {
                info!("Creating new character: {}", &id);
                let primary_entity = create_new_character(&mut commands, info);
                all_players.insert(id, primary_entity);
                commands.entity(primary_entity)
                    .insert((
                        UniqueId { id },
                    ));
                primary_entity
            }
        };

        commands.entity(primary_entity).insert(OwningClient { client_entity });
        commands.entity(client_entity).insert(Possessing { entity: primary_entity });
        info!("Attached character for {:?} = {:?}", client_entity, primary_entity);
    }
}

pub struct AccountsPlugin<T: AccountRepository>(PhantomData<T>);

impl<T: AccountRepository> Default for AccountsPlugin<T> {
    fn default() -> Self {
        AccountsPlugin(PhantomData)
    }
}

impl<T: AccountRepository> Plugin for AccountsPlugin<T> {
    fn build(&self, app: &mut App) {
        app
            .register_type::<NewCharacterInfo>()
            .init_resource::<PendingCharacterLists>()
            .init_resource::<PendingCharacterInfo>()
            .add_systems(First, (
                (
                    on_list_characters::<T>,
                    on_create_character::<T>,
                    on_select_character::<T>,
                    on_delete_character::<T>,
                ).in_set(ServerSet::HandlePackets),
            ))
            .add_systems(Update, (
                handle_list_characters_callback,
                handle_spawn_character::<T>,
            ));
    }
}
