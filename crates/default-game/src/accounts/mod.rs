use std::collections::HashMap;
use std::marker::PhantomData;
use std::time::Duration;

use bevy_app::{App, CoreSet, Plugin};
use bevy_ecs::prelude::*;
use glam::{IVec2, IVec3};
use tokio::sync::mpsc;
use uuid::Uuid;

use yewoh::{Direction, Notoriety};
use yewoh::protocol::{CharacterFromList, CharacterList, CharacterListFlags, EntityFlags, EntityTooltipLine, EquipmentSlot};
use yewoh::types::FixedString;
use yewoh_server::async_runtime::AsyncRuntime;
use yewoh_server::world::entity::{Character, CharacterEquipped, Container, EquippedBy, Flags, Graphic, Location, Notorious, ParentContainer, Stats, Tooltip};
use yewoh_server::world::events::{CharacterListEvent, CreateCharacterEvent, DeleteCharacterEvent, SelectCharacterEvent};
use yewoh_server::world::net::{NetClient, NetCommandsExt, NetOwner, Possessing, User};

use crate::accounts::repository::{AccountCharacters, AccountRepository, CharacterInfo, CharacterToSpawn};
use crate::activities::CurrentActivity;
use crate::characters::{Alive, Animation, MeleeWeapon, PredefinedAnimation, Unarmed};
use crate::data::static_data::StaticData;
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

pub fn handle_list_characters<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    account_repository: Res<T>,
    users: Query<&User>,
    pending: Res<PendingCharacterLists>,
    mut events: EventReader<CharacterListEvent>,
) {
    for event in events.iter() {
        let user = match users.get(event.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let username = user.username.clone();
        let tx = pending.tx.clone();
        let entity = event.client_entity;
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
    players_query: Query<(Entity, &UniqueId, &Stats), With<Character>>,
    mut all_players: Local<HashMap<Uuid, (Entity, String)>>,
) {
    let mut first = true;

    while let Ok((entity, result)) = pending.rx.try_recv() {
        let client = match clients.get(entity) {
            Ok(x) => x,
            _ => continue,
        };

        if first {
            first = false;
            all_players.clear();
            all_players.extend(players_query.iter()
                .map(|(entity, pc, stats)| (pc.id, (entity, stats.name.clone()))));
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
                    log::debug!("existing player id={} e={:?} n={}", player, &id.0, &id.1);
                }

                let character_list = CharacterList {
                    characters: characters.into_iter()
                        .map(|c|
                            c.and_then(|c| all_players.get(&c.id))
                                .map(|(_, name)| CharacterFromList {
                                    name: FixedString::from_str(&name),
                                    ..Default::default()
                                }))
                        .collect(),
                    cities: static_data.cities.to_starting_cities(),
                    flags,
                };

                client.send_packet(character_list.into());
            }
            Err(err) => log::warn!("Failed to list characters: {err}"),
        }
    }
}

pub fn handle_create_character<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    repository: Res<T>,
    users: Query<&User>,
    pending: Res<PendingCharacterInfo>,
    mut events: EventReader<CreateCharacterEvent>,
) {
    for CreateCharacterEvent { client_entity, request } in events.iter() {
        let client_entity = *client_entity;
        let user = match users.get(client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let repository = repository.clone();
        let username = user.username.clone();
        let request = request.clone();
        let tx = pending.tx.clone();
        runtime.spawn(async move {
            tx.send((client_entity, repository.create_character(&username, request).await)).ok();
        });
    }
}

pub fn handle_select_character<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    repository: Res<T>,
    users: Query<&User>,
    pending: Res<PendingCharacterInfo>,
    mut events: EventReader<SelectCharacterEvent>,
) {
    for SelectCharacterEvent { client_entity, request } in events.iter() {
        let client_entity = *client_entity;
        let user = match users.get(client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let repository = repository.clone();
        let username = user.username.clone();
        let tx = pending.tx.clone();
        let character_index = request.character_index as i32;
        runtime.spawn(async move {
            tx.send((client_entity, repository.load_character(&username, character_index).await)).ok();
        });
    }
}

pub fn handle_delete_character<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    repository: Res<T>,
    users: Query<&User>,
    mut events: EventReader<DeleteCharacterEvent>,
    pending: ResMut<PendingCharacterLists>,
) {
    for DeleteCharacterEvent { client_entity, request } in events.iter() {
        let client_entity = *client_entity;
        let user = match users.get(client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        let repository = repository.clone();
        let username = user.username.clone();
        let request = request.clone();
        let tx = pending.tx.clone();
        runtime.spawn(async move {
            if let Err(err) = repository.delete_character(&username, request).await {
                log::warn!("failed to delete character: {err}");
            }

            tx.send((client_entity, repository.list_characters(&username).await)).ok();
        });
    }
}

pub fn create_new_character(commands: &mut Commands, info: CharacterInfo) -> Entity {
    let body_type = match (info.race, info.is_female) {
        (1, false) => 605,
        (1, true) => 606,
        (2, false) => 666,
        (2, true) => 6667,
        (_, false) => 400,
        (_, true) => 401,
    };

    let bottom_graphic = match info.is_female {
        true => 0x1516,
        false => 0x1539,
    };

    let child_entity = commands.spawn((
        Flags { flags: EntityFlags::default() },
        Graphic { id: 0x9b2, hue: 120 },
        Container { gump_id: 0x3c, items: vec![] },
        Tooltip {
            entries: vec![
                EntityTooltipLine { text_id: 1042971, params: "Hello, world!".into() },
            ],
        },
        Persistent,
    )).id();

    let knife_entity = commands
        .spawn((
            Flags { flags: EntityFlags::default() },
            Graphic { id: 0xec3, hue: 16 },
            Tooltip {
                entries: vec![
                    EntityTooltipLine { text_id: 1042971, params: "Stabby stabby".into() },
                ],
            },
            MeleeWeapon {
                damage: 10,
                delay: Duration::from_secs(3),
                range: 4,
                swing_animation: Animation::Predefined(PredefinedAnimation {
                    kind: 0,
                    action: 4,
                    variant: 0,
                }),
            },
            Persistent,
        ))
        .id();

    let backpack_entity = commands.spawn((
        Flags::default(),
        Graphic { id: 0xe75, hue: 0 },
        Container { gump_id: 0x3c, items: vec![child_entity] },
        Persistent,
    )).id();
    let top_entity = commands.spawn((
        Flags::default(),
        Graphic { id: 0x1517, hue: info.shirt_hue },
        Persistent,
    )).id();
    let bottom_entity = commands.spawn((
        Flags::default(),
        Graphic { id: bottom_graphic, hue: info.pants_hue },
        Persistent,
    )).id();
    let shoes_entity = commands.spawn((
        Flags::default(),
        Graphic { id: 0x170f, hue: 0 },
        Persistent,
    )).id();

    let mut equipment = vec![
        (EquipmentSlot::Backpack, backpack_entity),
        (EquipmentSlot::Top, top_entity),
        (EquipmentSlot::Bottom, bottom_entity),
        (EquipmentSlot::Shoes, shoes_entity),
        (EquipmentSlot::MainHand, knife_entity),
    ];

    if info.hair != 0 {
        let entity = commands.spawn((
            Flags::default(),
            Graphic { id: info.hair, hue: info.hair_hue },
            Persistent,
        )).id();
        equipment.push((EquipmentSlot::Hair, entity));
    }

    if info.beard != 0 {
        let entity = commands.spawn((
            Flags::default(),
            Graphic { id: info.beard, hue: info.beard_hue },
            Persistent,
        )).id();
        equipment.push((EquipmentSlot::FacialHair, entity));
    }

    commands.entity(child_entity)
        .insert(ParentContainer {
            parent: backpack_entity,
            position: IVec2::new(0, 0),
            grid_index: 0,
        });

    let primary_entity = commands
        .spawn((
            Flags { flags: EntityFlags::default() },
            Location {
                map_id: 1,
                position: IVec3::new(1325, 1624, 55),
                direction: Direction::North,
            },
            Character {
                body_type,
                hue: info.hue,
                equipment: equipment.iter()
                    .copied()
                    .map(|(slot, equipment)|
                        CharacterEquipped { equipment, slot })
                    .collect(),
            },
            Notorious(Notoriety::Innocent),
            info.stats,
            Unarmed {
                weapon: MeleeWeapon {
                    damage: 1,
                    delay: Duration::from_secs(2),
                    range: 3,
                    swing_animation: Animation::Predefined(PredefinedAnimation {
                        kind: 0,
                        action: 0,
                        variant: 0,
                    }),
                },
            },
            Alive,
            CurrentActivity::Idle,
            Persistent,
        ))
        .id();

    for (slot, equipment_entity) in equipment {
        commands.entity(equipment_entity)
            .insert(EquippedBy { parent: primary_entity, slot });
    }

    primary_entity
}

pub fn handle_spawn_character<T: AccountRepository>(
    runtime: Res<AsyncRuntime>,
    mut pending: ResMut<PendingCharacterInfo>,
    pending_list: ResMut<PendingCharacterLists>,
    mut commands: Commands,
    existing_players_query: Query<(Entity, &UniqueId), With<Character>>,
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
                log::warn!("While spawning character: {err}");
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
                log::info!("Attaching to existing character: {}", &id);
                if let Some(character_entity) = all_players.get(&id).copied() {
                    character_entity
                } else {
                    log::warn!("Failed to connect to existing character: {}", &id);
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
                log::info!("Creating new character: {}", &id);
                let primary_entity = create_new_character(&mut commands, info);
                all_players.insert(id, primary_entity);
                commands.entity(primary_entity)
                    .insert(UniqueId { id })
                    .assign_network_id();
                primary_entity
            }
        };

        commands.entity(primary_entity).insert(NetOwner { client_entity });
        commands.entity(client_entity).insert(Possessing { entity: primary_entity });
        log::info!("Attached character for {:?} = {:?}", client_entity, primary_entity);
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
            .init_resource::<PendingCharacterLists>()
            .init_resource::<PendingCharacterInfo>()
            .add_systems((
                handle_list_characters::<T>,
                handle_list_characters_callback,
                handle_create_character::<T>,
                handle_select_character::<T>,
                handle_delete_character::<T>,
                handle_spawn_character::<T>,
            ).in_base_set(CoreSet::Update));
    }
}
