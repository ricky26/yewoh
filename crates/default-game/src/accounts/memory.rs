use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use bevy::ecs::system::Resource;
use tokio::sync::Mutex;
use uuid::Uuid;

use yewoh::protocol::{CreateCharacter, DeleteCharacter};

use crate::accounts::DEFAULT_CHARACTER_SLOTS;
use crate::accounts::repository::{AccountCharacter, AccountCharacters, AccountRepository, CharacterInfo, CharacterToSpawn};
use crate::entities::new_uuid;

#[derive(Debug, Clone, Default)]
struct MemoryUser {
    characters: Vec<Uuid>,
}

#[derive(Debug, Clone, Default)]
struct LockedMemoryAccountRepository {
    users: HashMap<String, MemoryUser>,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct MemoryAccountRepository {
    locked: Arc<Mutex<LockedMemoryAccountRepository>>,
}

#[async_trait]
impl AccountRepository for MemoryAccountRepository {
    async fn list_characters(&self, username: &str) -> anyhow::Result<AccountCharacters> {
        let mut locked = self.locked.lock().await;
        let user = locked.users.entry(username.to_string())
            .or_insert_with(Default::default);
        let padding = DEFAULT_CHARACTER_SLOTS - user.characters.len();
        let characters = user.characters.iter()
            .map(|id| AccountCharacter {
                id: *id,
            })
            .map(Some)
            .chain((0..padding).map(|_| None))
            .collect::<Vec<_>>();

        Ok(characters)
    }

    async fn create_character(&self, username: &str, request: CreateCharacter) -> anyhow::Result<CharacterToSpawn> {
        let info = CharacterInfo::from_request(&request);
        let mut locked = self.locked.lock().await;
        let user = locked.users.entry(username.to_string())
            .or_insert_with(Default::default);
        let id = new_uuid();
        user.characters.push(id);
        Ok(CharacterToSpawn::NewCharacter(id, info))
    }

    async fn delete_character(&self, _username: &str, _request: DeleteCharacter) -> anyhow::Result<()> {
        Ok(())
    }

    async fn load_character(&self, username: &str, slot: i32) -> anyhow::Result<CharacterToSpawn> {
        let mut locked = self.locked.lock().await;
        let user = locked.users.entry(username.to_string())
            .or_insert_with(Default::default);
        if user.characters.len() as i32 <= slot {
            return Err(anyhow::anyhow!("{username} has no character {slot}"));
        }
        Ok(CharacterToSpawn::ExistingCharacter(user.characters[slot as usize]))
    }
}
