use bevy_ecs::prelude::*;
use glam::IVec3;
use yewoh::{Direction, Notoriety};

use yewoh::protocol::{CharacterFromList, CharacterList, EquipmentSlot, UnicodeTextMessage};
use yewoh_server::world::client::{NetClients};
use yewoh_server::world::entity::{Character, EquippedBy, Graphic, HasNotoriety, MapPosition, NetEntity, NetEntityAllocator, Stats};
use yewoh_server::world::events::{CharacterListEvent, CreateCharacterEvent, NewPrimaryEntityEvent};

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
    server: Res<NetClients>,
    //account_repository: Res<T>,
    //users: Query<&User>,
    mut events: EventReader<CharacterListEvent>,
) {
    for event in events.iter() {
        /*let user = match users.get(event.connection) {
            Ok(x) => x,
            Err(_) => continue,
        };*/

        let connection = event.connection;

        server.send_packet(connection, CharacterList {
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
    mut events: EventReader<CreateCharacterEvent>,
    mut out_events: EventWriter<NewPrimaryEntityEvent>,
    mut commands: Commands,
    server: Res<NetClients>,
) {
    for event in events.iter() {
        let connection = event.connection;

        let backpack_id = entity_allocator.allocate_item();
        let backpack_entity = commands.spawn()
            .insert(NetEntity { id: backpack_id })
            .insert(Graphic { id: 0x9b2, hue: 120 })
            .id();

        let primary_entity_id = entity_allocator.allocate_character();
        let primary_entity = commands.spawn()
            .insert(NetEntity { id: primary_entity_id })
            .insert(MapPosition {
                map_id: 1,
                position: IVec3::new(2000, 2000, 0),
                direction: Direction::North,
            })
            .insert(Character {
                body_type: 0x25e,
                hue: 120,
                equipment: vec![ backpack_entity ],
            })
            .insert(HasNotoriety(Notoriety::Innocent))
            .insert(Stats {
                name: "Wise Dave".into(),
                hp: 500,
                max_hp: 600,
                ..Default::default()
            })
            .id();

        commands.entity(backpack_entity)
            .insert(EquippedBy { entity: primary_entity, slot: EquipmentSlot::Backpack });

        out_events.send(NewPrimaryEntityEvent { connection, primary_entity: Some(primary_entity) });
        server.send_packet(connection, UnicodeTextMessage {
            text: "Avast me hearties".to_string(),
            hue: 120,
            font: 3,
            ..Default::default()
        }.into());
        log::info!("Spawned character for {:?} = {:?}", connection, primary_entity);
    }
}
