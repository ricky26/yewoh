use std::sync::Arc;
use bevy::ecs::system::Resource;
use sqlx::{FromRow, PgPool};

#[derive(FromRow)]
pub struct WorldRow {
    pub snapshot: Vec<u8>,
}

struct WorldRepositoryInner {
    pool: Arc<PgPool>,
    shard_id: String,
}

#[derive(Resource, Clone)]
pub struct WorldRepository {
    inner: Arc<WorldRepositoryInner>,
}

impl WorldRepository {
    pub fn new(pool: Arc<PgPool>, shard_id: String) -> Self {
        Self {
            inner: Arc::new(WorldRepositoryInner {
                pool,
                shard_id,
            }),
        }
    }

    pub async fn get_snapshot(&self) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(sqlx::query_as("SELECT snapshot FROM world_snapshots WHERE shard_id = $1 LIMIT 1")
            .bind(self.inner.shard_id.as_str())
            .fetch_optional(self.inner.pool.as_ref())
            .await?
            .map(|v: WorldRow| v.snapshot))
    }

    pub async fn put_snapshot(&self, snapshot: Vec<u8>) -> anyhow::Result<()> {
        sqlx::query(r#"
                INSERT INTO world_snapshots (shard_id, snapshot, updated_at)
                VALUES ($1, $2, CURRENT_TIMESTAMP)
                ON CONFLICT (shard_id) DO UPDATE
                SET snapshot = EXCLUDED.snapshot, updated_at = CURRENT_TIMESTAMP"#)
            .bind(&self.inner.shard_id)
            .bind(&snapshot)
            .execute(self.inner.pool.as_ref())
            .await?;
        Ok(())
    }
}
