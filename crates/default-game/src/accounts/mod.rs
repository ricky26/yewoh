use std::time::Duration;
use bevy_app::{App, CoreSet, Plugin};
use bevy_ecs::prelude::*;
use glam::{IVec2, IVec3};
use tokio::sync::mpsc;

use yewoh::{Direction, Notoriety};
use yewoh::protocol::{CharacterList, CharacterListFlags, EntityFlags, EntityTooltipLine, EquipmentSlot};
use yewoh_server::async_runtime::AsyncRuntime;
use yewoh_server::world::entity::{Character, CharacterEquipped, Container, EquippedBy, Flags, Graphic, Location, Notorious, ParentContainer, Tooltip};
use yewoh_server::world::events::{CharacterListEvent, CreateCharacterEvent, SelectCharacterEvent};
use yewoh_server::world::net::{NetClient, NetEntity, NetEntityAllocator, NetOwner, Possessing, User};

use crate::accounts::repository::{AccountRepository, CharacterInfo};
use crate::activities::CurrentActivity;
use crate::characters::{Alive, Animation, MeleeWeapon, PredefinedAnimation, Unarmed};
use crate::data::static_data::StaticData;

pub mod repository;

#[derive(Resource)]
pub struct PendingCharacterLists {
    tx: mpsc::UnboundedSender<(Entity, anyhow::Result<CharacterList>)>,
    rx: mpsc::UnboundedReceiver<(Entity, anyhow::Result<CharacterList>)>,
}

impl Default for PendingCharacterLists {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { tx, rx }
    }
}

#[derive(Resource)]
pub struct PendingCharacterInfo {
    tx: mpsc::UnboundedSender<(Entity, anyhow::Result<CharacterInfo>)>,
    rx: mpsc::UnboundedReceiver<(Entity, anyhow::Result<CharacterInfo>)>,
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
) {
    while let Ok((entity, result)) = pending.rx.try_recv() {
        let client = match clients.get(entity) {
            Ok(x) => x,
            _ => continue,
        };

        match result {
            Ok(mut characters) => {
                characters.cities = static_data.cities.to_starting_cities();
                characters.flags |= CharacterListFlags::ALLOW_OVERWRITE_CONFIG
                    | CharacterListFlags::CONTEXT_MENU
                    | CharacterListFlags::PALADIN_NECROMANCER_TOOLTIPS
                    | CharacterListFlags::SAMURAI_NINJA
                    | CharacterListFlags::ELVES
                    | CharacterListFlags::NEW_MOVEMENT_SYSTEM
                    | CharacterListFlags::ALLOW_FELUCCA;

                if characters.characters.len() > 6 {
                    characters.flags |= CharacterListFlags::SEVENTH_CHARACTER_SLOT;
                }

                if characters.characters.len() > 5 {
                    characters.flags |= CharacterListFlags::SIXTH_CHARACTER_SLOT;
                }

                if characters.characters.len() == 1 {
                    characters.flags |= CharacterListFlags::SLOT_LIMIT
                        | CharacterListFlags::SINGLE_CHARACTER_SLOT;
                }

                client.send_packet(characters.into());
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
        let character_name = request.name.clone();
        let tx = pending.tx.clone();
        runtime.spawn(async move {
            tx.send((client_entity, repository.load_character(&username, &character_name).await)).ok();
        });
    }
}

pub fn handle_spawn_character(
    entity_allocator: Res<NetEntityAllocator>,
    mut pending: ResMut<PendingCharacterInfo>,
    mut commands: Commands,
) {
    while let Ok((entity, result)) = pending.rx.try_recv() {
        let client_entity = entity;
        let info = match result {
            Ok(x) => x,
            Err(err) => {
                log::warn!("While spawning character: {err}");
                continue;
            }
        };

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

        let child_backpack_id = entity_allocator.allocate_item();
        let child_entity = commands.spawn((
            NetEntity { id: child_backpack_id },
            Flags { flags: EntityFlags::default() },
            Graphic { id: 0x9b2, hue: 120 },
            Container { gump_id: 0x3c, items: vec![] },
            Tooltip {
                entries: vec![
                    EntityTooltipLine { text_id: 1042971, params: "Hello, world!".into() },
                ],
            }))
            .id();

        let knife_entity = commands
            .spawn((
                NetEntity { id: entity_allocator.allocate_item() },
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
            ))
            .id();

        let backpack_entity = commands.spawn((
            NetEntity { id: entity_allocator.allocate_item() },
            Flags::default(),
            Graphic { id: 0xe75, hue: 0 },
            Container { gump_id: 0x3c, items: vec![child_entity] }))
            .id();
        let top_entity = commands.spawn((
            NetEntity { id: entity_allocator.allocate_item() },
            Flags::default(),
            Graphic { id: 0x1517, hue: info.shirt_hue }))
            .id();
        let bottom_entity = commands.spawn((
            NetEntity { id: entity_allocator.allocate_item() },
            Flags::default(),
            Graphic { id: bottom_graphic, hue: info.pants_hue }))
            .id();
        let shoes_entity = commands.spawn((
            NetEntity { id: entity_allocator.allocate_item() },
            Flags::default(),
            Graphic { id: 0x170f, hue: 0 }))
            .id();

        let mut equipment = vec![
            (EquipmentSlot::Backpack, backpack_entity),
            (EquipmentSlot::Top, top_entity),
            (EquipmentSlot::Bottom, bottom_entity),
            (EquipmentSlot::Shoes, shoes_entity),
            (EquipmentSlot::MainHand, knife_entity),
        ];

        if info.hair != 0 {
            let entity = commands.spawn((
                NetEntity { id: entity_allocator.allocate_item() },
                Flags::default(),
                Graphic { id: info.hair, hue: info.hair_hue }))
                .id();
            equipment.push((EquipmentSlot::Hair, entity));
        }

        if info.beard != 0 {
            let entity = commands.spawn((
                NetEntity { id: entity_allocator.allocate_item() },
                Flags::default(),
                Graphic { id: info.beard, hue: info.beard_hue }))
                .id();
            equipment.push((EquipmentSlot::FacialHair, entity));
        }

        commands.entity(child_entity)
            .insert(ParentContainer {
                parent: backpack_entity,
                position: IVec2::new(1, 0),
                grid_index: 1,
            });

        let primary_entity_id = entity_allocator.allocate_character();
        let primary_entity = commands
            .spawn((
                NetEntity { id: primary_entity_id },
                NetOwner { client_entity },
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
            ))
            .id();

        for (slot, equipment_entity) in equipment {
            commands.entity(equipment_entity)
                .insert(EquippedBy { parent: primary_entity, slot });
        }

        commands.entity(client_entity).insert(Possessing { entity: primary_entity });
        log::info!("Spawned character for {:?} = {:?}", client_entity, primary_entity);
    }
}

#[derive(Default)]
pub struct AccountsPlugin;

impl Plugin for AccountsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<repository::MemoryAccountRepository>()
            .init_resource::<PendingCharacterLists>()
            .init_resource::<PendingCharacterInfo>()
            .add_systems((
                handle_list_characters::<repository::MemoryAccountRepository>,
                handle_list_characters_callback,
                handle_create_character::<repository::MemoryAccountRepository>,
                handle_select_character::<repository::MemoryAccountRepository>,
                handle_spawn_character,
            ).in_base_set(CoreSet::Update));
    }
}
