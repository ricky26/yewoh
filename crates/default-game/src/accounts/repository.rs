use async_trait::async_trait;
use bevy::prelude::*;
use uuid::Uuid;

use yewoh::protocol::{CreateCharacter, DeleteCharacter};
use yewoh_server::world::characters::Stats;

#[derive(Debug, Clone, Default, Reflect)]
#[reflect(Default)]
pub struct CharacterInfo {
    pub hue: u16,
    pub hair: u16,
    pub hair_hue: u16,
    pub beard: u16,
    pub beard_hue: u16,
    pub shirt_hue: u16,
    pub pants_hue: u16,
    pub stats: Stats,
}

impl CharacterInfo {
    pub fn from_request(request: &CreateCharacter) -> Self {
        Self {
            hue: request.hue,
            hair: request.hair.graphic,
            hair_hue: request.hair.hue,
            beard: request.beard.graphic,
            beard_hue: request.beard.hue,
            shirt_hue: request.shirt_hue,
            pants_hue: request.pants_hue,
            stats: Stats {
                name: request.character_name.to_string(),
                female: request.is_female,
                race: request.race,
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
        }
    }
}

#[derive(Debug, Clone)]
pub enum CharacterToSpawn {
    NewCharacter(Uuid, CharacterInfo),
    ExistingCharacter(Uuid),
}

#[derive(Debug, Clone)]
pub struct AccountCharacter {
    pub id: Uuid,
}

pub type AccountCharacters = Vec<Option<AccountCharacter>>;

#[async_trait]
pub trait AccountRepository: Clone + Resource {
    async fn list_characters(&self, username: &str) -> anyhow::Result<AccountCharacters>;
    async fn create_character(&self, username: &str, request: CreateCharacter) -> anyhow::Result<CharacterToSpawn>;
    async fn delete_character(&self, username: &str, request: DeleteCharacter) -> anyhow::Result<()>;
    async fn load_character(&self, username: &str, slot: i32) -> anyhow::Result<CharacterToSpawn>;
}
