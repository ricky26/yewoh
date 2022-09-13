use bevy_ecs::prelude::*;
use glam::{IVec2, IVec3};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use yewoh::{Direction, Notoriety};
use yewoh::protocol::{CharacterList, EntityFlags, EquipmentSlot, UnicodeTextMessage};
use yewoh_server::world::entity::{Character, Container, EquippedBy, Flags, Graphic, MapPosition, Notorious, ParentContainer};
use yewoh_server::world::events::{CharacterListEvent, CreateCharacterEvent, NewPrimaryEntityEvent, SelectCharacterEvent};
use yewoh_server::world::net::{NetClient, NetEntity, NetEntityAllocator, User};

use crate::accounts::repository::{AccountRepository, CharacterInfo};
use crate::data::static_data::StaticData;

pub mod repository;

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
    runtime: Res<Handle>,
    account_repository: Res<T>,
    users: Query<&User>,
    pending: Res<PendingCharacterLists>,
    mut events: EventReader<CharacterListEvent>,
) {
    for event in events.iter() {
        let user = match users.get(event.client) {
            Ok(x) => x,
            _ => continue,
        };

        let username = user.username.clone();
        let tx = pending.tx.clone();
        let entity = event.client;
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
                client.send_packet(characters.into());
            }
            Err(err) => log::warn!("Failed to list characters: {err}"),
        }
    }
}

pub fn handle_create_character<T: AccountRepository>(
    runtime: Res<Handle>,
    repository: Res<T>,
    users: Query<&User>,
    pending: Res<PendingCharacterInfo>,
    mut events: EventReader<CreateCharacterEvent>,
) {
    for CreateCharacterEvent { client: client_entity, request} in events.iter() {
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
    runtime: Res<Handle>,
    repository: Res<T>,
    users: Query<&User>,
    pending: Res<PendingCharacterInfo>,
    mut events: EventReader<SelectCharacterEvent>,
) {
    for SelectCharacterEvent { client: client_entity, request } in events.iter() {
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
    clients: Query<&NetClient>,
    mut pending: ResMut<PendingCharacterInfo>,
    mut out_events: EventWriter<NewPrimaryEntityEvent>,
    mut commands: Commands,
) {
    while let Ok((entity, result)) = pending.rx.try_recv() {
        let client_entity = entity;
        let client = match clients.get(client_entity) {
            Ok(x) => x,
            _ => continue,
        };

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
        let child_entity = commands.spawn()
            .insert(NetEntity { id: child_backpack_id })
            .insert(Flags { flags: EntityFlags::default() })
            .insert(Graphic { id: 0x9b2, hue: 120 })
            .insert(Container { gump_id: 0x3c, items: vec![] })
            .id();

        let backpack_entity = commands.spawn()
            .insert(NetEntity { id: entity_allocator.allocate_item() })
            .insert(Flags::default())
            .insert(Graphic { id: 0xe75, hue: 0 })
            .insert(Container { gump_id: 0x3c, items: vec![child_entity] })
            .id();
        let top_entity = commands.spawn()
            .insert(NetEntity { id: entity_allocator.allocate_item() })
            .insert(Flags::default())
            .insert(Graphic { id: 0x1517, hue: info.shirt_hue })
            .id();
        let bottom_entity = commands.spawn()
            .insert(NetEntity { id: entity_allocator.allocate_item() })
            .insert(Flags::default())
            .insert(Graphic { id: bottom_graphic, hue: info.pants_hue })
            .id();
        let shoes_entity = commands.spawn()
            .insert(NetEntity { id: entity_allocator.allocate_item() })
            .insert(Flags::default())
            .insert(Graphic { id: 0x170f, hue: 0 })
            .id();

        let mut equipment = vec![
            (EquipmentSlot::Backpack, backpack_entity),
            (EquipmentSlot::Top, top_entity),
            (EquipmentSlot::Bottom, bottom_entity),
            (EquipmentSlot::Shoes, shoes_entity),
        ];

        if info.hair != 0 {
            let entity = commands.spawn()
                .insert(NetEntity { id: entity_allocator.allocate_item() })
                .insert(Flags::default())
                .insert(Graphic { id: info.hair, hue: info.hair_hue })
                .id();
            equipment.push((EquipmentSlot::Hair, entity));
        }

        if info.beard != 0 {
            let entity = commands.spawn()
                .insert(NetEntity { id: entity_allocator.allocate_item() })
                .insert(Flags::default())
                .insert(Graphic { id: info.beard, hue: info.beard_hue })
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
        let primary_entity = commands.spawn()
            .insert(NetEntity { id: primary_entity_id })
            .insert(Flags { flags: EntityFlags::default() })
            .insert(MapPosition {
                map_id: 1,
                position: IVec3::new(1325, 1624, 55),
                direction: Direction::North,
            })
            .insert(Character {
                body_type,
                hue: info.hue,
                equipment: equipment.iter().map(|(_, e)| e).copied().collect(),
            })
            .insert(Notorious(Notoriety::Innocent))
            .insert(info.stats)
            .id();

        for (slot, equipment_entity) in equipment {
            commands.entity(equipment_entity)
                .insert(EquippedBy { parent: primary_entity, slot });
        }

        out_events.send(NewPrimaryEntityEvent { client: client_entity, primary_entity: Some(primary_entity) });
        client.send_packet(UnicodeTextMessage {
            text: "Avast me hearties".to_string(),
            hue: 120,
            font: 3,
            ..Default::default()
        }.into());
        log::info!("Spawned character for {:?} = {:?}", client_entity, primary_entity);
    }
}
