use std::sync::Arc;
use bevy_ecs::system::Resource;
use sqlx::{FromRow, PgPool};

#[derive(FromRow)]
pub struct WorldRow {
    pub shard_id: String,
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
            .bind(&self.inner.shard_id)
            .fetch_optional(self.inner.pool.as_ref())
            .await?
            .map(|v: WorldRow| v.snapshot))
    }

    pub async fn put_snapshot(&self, snapshot: Vec<u8>) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO world_snapshots (shard_id, snapshot) VALUES ($1, $2) ON CONFLICT UPDATE")
            .bind(&self.inner.shard_id)
            .bind(&snapshot)
            .execute(self.inner.pool.as_ref())
            .await?;
        Ok(())
    }
}
