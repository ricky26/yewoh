use std::collections::HashMap;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use bevy_ecs::system::Resource;
use tokio::sync::Mutex;

use yewoh::protocol::{CharacterFromList, CharacterList, CreateCharacter};
use yewoh_server::world::entity::Stats;

#[derive(Debug, Clone)]
pub struct CharacterInfo {
    pub race: u8,
    pub hue: u16,
    pub is_female: bool,
    pub hair: u16,
    pub hair_hue: u16,
    pub beard: u16,
    pub beard_hue: u16,
    pub shirt_hue: u16,
    pub pants_hue: u16,
    pub stats: Stats,
}

#[async_trait]
pub trait AccountRepository: Clone + Resource {
    async fn list_characters(&self, username: &str) -> anyhow::Result<CharacterList>;
    async fn create_character(&self, username: &str, request: CreateCharacter) -> anyhow::Result<CharacterInfo>;
    async fn load_character(&self, username: &str, name: &str) -> anyhow::Result<CharacterInfo>;
}

const MAX_CHARACTERS: usize = 6;

#[derive(Debug, Clone, Default)]
struct MemoryUser {
    characters: HashMap<String, CharacterInfo>,
}

#[derive(Debug, Clone, Default)]
struct LockedMemoryAccountRepository {
    users: HashMap<String, MemoryUser>,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryAccountRepository {
    locked: Arc<Mutex<LockedMemoryAccountRepository>>,
}

#[async_trait]
impl AccountRepository for MemoryAccountRepository {
    async fn list_characters(&self, username: &str) -> anyhow::Result<CharacterList> {
        let mut locked = self.locked.lock().await;
        let user = locked.users.entry(username.to_string())
            .or_insert_with(|| Default::default());
        let padding = MAX_CHARACTERS - user.characters.len();
        let characters = user.characters.keys()
            .map(|name| CharacterFromList {
                name: name.clone(),
                ..Default::default()
            })
            .map(Some)
            .chain((0..padding).map(|_| None))
            .collect::<Vec<_>>();

        Ok(CharacterList {
            characters,
            cities: Vec::new(),
        })
    }

    async fn create_character(&self, username: &str, request: CreateCharacter) -> anyhow::Result<CharacterInfo> {
        let info = CharacterInfo {
            race: request.race,
            hue: request.hue,
            is_female: request.is_female,
            hair: request.hair.graphic,
            hair_hue: request.hair.hue,
            beard: request.beard.graphic,
            beard_hue: request.beard.hue,
            shirt_hue: request.shirt_hue,
            pants_hue: request.pants_hue,
            stats: Stats {
                name: request.character_name.clone(),
                race_and_gender: (request.race << 1) | if request.is_female { 1 } else { 0 },
                str: request.str as u16,
                dex: request.dex as u16,
                int: request.int as u16,
                hp: 500,
                max_hp: 500,
                mana: 500,
                max_mana: 500,
                stamina: 500,
                max_stamina: 500,
                ..Default::default()
            },
        };

        let mut locked = self.locked.lock().await;
        let user = locked.users.entry(username.to_string())
            .or_insert_with(|| Default::default());
        user.characters.insert(request.character_name.clone(), info.clone());
        Ok(info)
    }

    async fn load_character(&self, username: &str, name: &str) -> anyhow::Result<CharacterInfo> {
        let mut locked = self.locked.lock().await;
        let user = locked.users.entry(username.to_string())
            .or_insert_with(|| Default::default());

        if let Some(info) = user.characters.get(name) {
            Ok(info.clone())
        } else {
            Err(anyhow!("No such character"))
        }
    }
}