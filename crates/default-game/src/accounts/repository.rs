use async_trait::async_trait;
use bevy::prelude::*;
use uuid::Uuid;
use yewoh::protocol;
use yewoh::protocol::{CreateCharacter, DeleteCharacter, Race};
use yewoh_server::world::characters::CharacterStats;

#[derive(Debug, Copy, Clone, Default, Reflect)]
pub enum NewCharacterProfession {
    #[default]
    Custom,
    Warrior,
    Magician,
    Blacksmith,
    Necromancer,
    Paladin,
    Samurai,
    Ninja,
}

impl From<protocol::NewCharacterProfession> for NewCharacterProfession {
    fn from(value: protocol::NewCharacterProfession) -> Self {
        match value {
            protocol::NewCharacterProfession::Custom => NewCharacterProfession::Custom,
            protocol::NewCharacterProfession::Warrior => NewCharacterProfession::Warrior,
            protocol::NewCharacterProfession::Magician => NewCharacterProfession::Magician,
            protocol::NewCharacterProfession::Blacksmith => NewCharacterProfession::Blacksmith,
            protocol::NewCharacterProfession::Necromancer => NewCharacterProfession::Necromancer,
            protocol::NewCharacterProfession::Paladin => NewCharacterProfession::Paladin,
            protocol::NewCharacterProfession::Samurai => NewCharacterProfession::Samurai,
            protocol::NewCharacterProfession::Ninja => NewCharacterProfession::Ninja,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Reflect)]
pub struct NewCharacterSkill {
    pub skill_id: u8,
    pub points: u8,
}

#[derive(Debug, Clone, Default, Reflect)]
#[reflect(Default)]
pub struct NewCharacterInfo {
    pub name: String,
    #[reflect(remote = yewoh_server::remote_reflect::Race)]
    pub race: Race,
    pub female: bool,
    pub hue: u16,
    pub hair: u16,
    pub hair_hue: u16,
    pub beard: u16,
    pub beard_hue: u16,
    pub shirt_hue: u16,
    pub pants_hue: u16,
    pub profession: NewCharacterProfession,
    pub skills: [NewCharacterSkill; 4],
    pub stats: CharacterStats,
    pub city_index: u16,
}

impl NewCharacterInfo {
    pub fn from_request(request: &CreateCharacter) -> Self {
        Self {
            name: request.character_name.to_string(),
            race: request.race,
            female: request.is_female,
            hue: request.hue,
            hair: request.hair.graphic,
            hair_hue: request.hair.hue,
            beard: request.beard.graphic,
            beard_hue: request.beard.hue,
            shirt_hue: request.shirt_hue,
            pants_hue: request.pants_hue,
            profession: request.profession.into(),
            skills: request.skills.map(|s| NewCharacterSkill {
                skill_id: s.skill_id,
                points: s.points,
            }),
            stats: CharacterStats {
                str: request.str as u16,
                dex: request.dex as u16,
                int: request.int as u16,
            },
            city_index: request.city_index,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CharacterToSpawn {
    NewCharacter(Uuid, NewCharacterInfo),
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
