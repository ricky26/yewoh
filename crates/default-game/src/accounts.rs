use bevy_ecs::prelude::*;
use glam::{IVec2, IVec3};
use yewoh::{Direction, Notoriety};

use yewoh::protocol::{CharacterFromList, CharacterList, EntityFlags, EquipmentSlot, UnicodeTextMessage};
use yewoh_server::world::entity::{Character, Container, EquippedBy, Graphic, Notorious, MapPosition, ParentContainer, Stats, Flags};
use yewoh_server::world::events::{CharacterListEvent, CreateCharacterEvent, NewPrimaryEntityEvent};
use yewoh_server::world::net::{NetClient, NetEntity, NetEntityAllocator};

use crate::data::static_data::StaticData;

/*
#[async_trait]
pub trait AccountRepository {
    async fn list_accounts_for_user(&self, username: &str) -> anyhow::Result<CharacterList>;
}
 */

pub fn handle_list_characters(
    //runtime: Res<Handle>,
    static_data: Res<StaticData>,
    //account_repository: Res<T>,
    //users: Query<&User>,
    clients: Query<&NetClient>,
    mut events: EventReader<CharacterListEvent>,
) {
    for event in events.iter() {
        /*let user = match users.get(event.connection) {
            Ok(x) => x,
            Err(_) => continue,
        };*/

        let client = match clients.get(event.client) {
            Ok(x) => x,
            _ => continue,
        };
        client.send_packet(CharacterList {
            characters: vec![
                Some(CharacterFromList {
                    name: "test".to_string(),
                    password: "123456".to_string(),
                }),
                None,
                None,
                None,
                None,
            ],
            cities: static_data.cities.to_starting_cities(),
        }.into());

        /*let username = user.username.clone();
        runtime.spawn(async move {
            match account_repository.list_accounts_for_user(&username).await {
                Ok(characters) =>
                    server.send_packet(connection, characters.into()),
                Err(err) => log::warn!("Failed to list characters: {err}"),
            }
        });*/
    }
}

pub fn handle_create_character(
    entity_allocator: Res<NetEntityAllocator>,
    clients: Query<&NetClient>,
    mut events: EventReader<CreateCharacterEvent>,
    mut out_events: EventWriter<NewPrimaryEntityEvent>,
    mut commands: Commands,
) {
    for event in events.iter() {
        let client_entity = event.client;
        let client = match clients.get(event.client) {
            Ok(x) => x,
            _ => continue,
        };

        let child_backpack_id = entity_allocator.allocate_item();
        let child_entity = commands.spawn()
            .insert(NetEntity {id: child_backpack_id})
            .insert(Flags { flags: EntityFlags::default() })
            .insert(Graphic {id: 0x9b2, hue: 120})
            .id();

        let backpack_id = entity_allocator.allocate_item();
        let backpack_entity = commands.spawn()
            .insert(NetEntity { id: backpack_id })
            .insert(Flags { flags: EntityFlags::default() })
            .insert(Graphic { id: 0x9b2, hue: 120 })
            .insert(Container { gump_id: 7, items: vec![child_entity] })
            .id();

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
                body_type: 0x25e,
                hue: 120,
                equipment: vec![ backpack_entity ],
            })
            .insert(Notorious(Notoriety::Innocent))
            .insert(Stats {
                name: "Wise Dave".into(),
                hp: 500,
                max_hp: 600,
                ..Default::default()
            })
            .id();

        commands.entity(backpack_entity)
            .insert(EquippedBy { parent: primary_entity, slot: EquipmentSlot::Backpack });

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
