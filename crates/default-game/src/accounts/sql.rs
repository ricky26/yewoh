use std::sync::Arc;

use anyhow::anyhow;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use async_trait::async_trait;
use bevy_ecs::system::Resource;
use futures::StreamExt;
use rand::thread_rng;
use sqlx::{FromRow, PgPool};
use tracing::warn;
use uuid::Uuid;

use yewoh::protocol::{CreateCharacter, DeleteCharacter};
use yewoh_server::lobby;

use crate::accounts::repository::{AccountCharacter, AccountCharacters, AccountRepository, CharacterInfo, CharacterToSpawn};
use crate::accounts::DEFAULT_CHARACTER_SLOTS;
use crate::entities::new_uuid;

#[derive(Debug, Clone)]
pub struct SqlAccountRepositoryConfig {
    pub auto_create_accounts: bool,
}

#[derive(Clone, FromRow)]
#[allow(dead_code)]
struct AccountDto {
    pub username: String,
    pub password_hash: String,
    pub character_slots: i32,
}

#[derive(Clone, FromRow)]
struct CharacterDto {
    pub id: Uuid,
    pub slot: i32,
}

pub struct SqlAccountRepositoryInner {
    config: SqlAccountRepositoryConfig,
    pool: Arc<PgPool>,
    password_hasher: Argon2<'static>,
}

#[derive(Clone, Resource)]
pub struct SqlAccountRepository {
    inner: Arc<SqlAccountRepositoryInner>,
}

impl SqlAccountRepository {
    pub fn new(config: SqlAccountRepositoryConfig, pool: Arc<PgPool>) -> Self {
        Self {
            inner: Arc::new(SqlAccountRepositoryInner {
                config,
                pool,
                password_hasher: Argon2::default(),
            }),
        }
    }

    async fn get_account_optional(&self, username: &str) -> anyhow::Result<Option<AccountDto>> {
        Ok(sqlx::query_as(r#"
            SELECT username, password_hash, character_slots
            FROM accounts
            WHERE username = $1
        "#)
            .bind(username)
            .fetch_optional(self.inner.pool.as_ref())
            .await?)
    }

    async fn get_account(&self, username: &str) -> anyhow::Result<AccountDto> {
        self.get_account_optional(username)
            .await?
            .ok_or_else(|| anyhow!("no such account"))
    }

    pub async fn create_account(&self, username: &str, password: &str) -> anyhow::Result<()> {
        let salt = SaltString::generate(&mut thread_rng());
        let hash = self.inner.password_hasher.hash_password(password.as_bytes(), salt.as_salt())?;
        let hash_str = hash.serialize();
        sqlx::query("INSERT INTO accounts (username, password_hash, character_slots) VALUES ($1, $2, $3)")
            .bind(username)
            .bind(hash_str.as_str())
            .bind(DEFAULT_CHARACTER_SLOTS as i32)
            .execute(self.inner.pool.as_ref())
            .await?;
        Ok(())
    }
}

#[async_trait]
impl AccountRepository for SqlAccountRepository {
    async fn list_characters(&self, username: &str) -> anyhow::Result<AccountCharacters> {
        let account = self.get_account(username).await?;
        let mut characters = vec![None; account.character_slots as usize];
        let mut fetched_characters = sqlx::query_as::<_, CharacterDto>(
            "SELECT id, slot FROM characters WHERE username = $1")
            .bind(username)
            .fetch(self.inner.pool.as_ref());
        while let Some(result) = fetched_characters.next().await {
            let character = result?;
            if character.slot < 0 || character.slot >= characters.len() as i32 {
                warn!("character {} is in invalid slot {}", &character.id, character.slot);
            } else {
                characters[character.slot as usize] = Some(AccountCharacter {
                    id: character.id,
                });
            }
        }

        Ok(characters)
    }

    async fn create_character(&self, username: &str, request: CreateCharacter) -> anyhow::Result<CharacterToSpawn> {
        let info = CharacterInfo::from_request(&request);
        let account = self.get_account(username).await?;
        let mut tx = self.inner.pool.begin().await?;

        let used_slots: Vec<(i32,)> = sqlx::query_as("SELECT slot FROM characters WHERE username = $1")
            .bind(username)
            .fetch_all(&mut *tx)
            .await?;
        let slot = match (0..account.character_slots).filter(|i| !used_slots.contains(&(*i,))).next() {
            Some(x) => x as i32,
            None => {
                tx.rollback().await?;
                return Err(anyhow!("no free slots"));
            }
        };

        let id = new_uuid();
        sqlx::query("INSERT INTO characters (id, username, slot) VALUES ($1, $2, $3)")
            .bind(&id)
            .bind(username)
            .bind(slot)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(CharacterToSpawn::NewCharacter(id, info))
    }

    async fn delete_character(&self, username: &str, request: DeleteCharacter) -> anyhow::Result<()> {
        let num_rows = sqlx::query("DELETE FROM characters WHERE username = $1 AND slot = $2")
            .bind(username)
            .bind(request.character_index as i32)
            .execute(self.inner.pool.as_ref())
            .await?
            .rows_affected();
        if num_rows != 1 {
            return Err(anyhow!("no such character exists (user={}, slot={})", username, request.character_index));
        }

        Ok(())
    }

    async fn load_character(&self, username: &str, slot: i32) -> anyhow::Result<CharacterToSpawn> {
        #[derive(FromRow)]
        struct Character {
            pub id: Uuid,
        }

        let character: Option<Character> = sqlx::query_as(
            "SELECT id FROM characters WHERE slot = $1 AND username = $2")
            .bind(slot)
            .bind(username)
            .fetch_optional(self.inner.pool.as_ref())
            .await?;
        if let Some(character) = character {
            Ok(CharacterToSpawn::ExistingCharacter(character.id))
        } else {
            Err(anyhow!("unable to log {username} in as slot {}", slot))
        }
    }
}

#[async_trait]
impl lobby::AccountRepository for SqlAccountRepository {
    async fn login(&mut self, username: &str, password: &str) -> anyhow::Result<()> {
        let account = self.get_account_optional(username).await?;
        let account: AccountDto = match account {
            Some(x) => x,
            None => {
                return if self.inner.config.auto_create_accounts {
                    self.create_account(username, password).await
                } else {
                    Err(anyhow!("invalid username or password"))
                };
            }
        };

        let hash = PasswordHash::new(&account.password_hash)?;
        if let Err(err) = self.inner.password_hasher.verify_password(password.as_bytes(), &hash) {
            warn!("bad login attempt: {err}");
            Err(anyhow!("invalid username or password"))
        } else {
            Ok(())
        }
    }
}
