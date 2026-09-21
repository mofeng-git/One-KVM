use sqlx::{Pool, Sqlite};

use crate::error::Result;

const MAX_ENTRIES: i64 = 50;

#[derive(Clone)]
pub struct WolHistoryStore {
    pool: Pool<Sqlite>,
}

impl WolHistoryStore {
    pub(crate) fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub async fn record(&self, mac_address: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO wol_history (mac_address, updated_at) VALUES (?1, CAST(strftime('%s', 'now') AS INTEGER)) ON CONFLICT(mac_address) DO UPDATE SET updated_at = excluded.updated_at")
            .bind(mac_address)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM wol_history WHERE mac_address NOT IN (SELECT mac_address FROM wol_history ORDER BY updated_at DESC LIMIT ?1)")
            .bind(MAX_ENTRIES)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list(&self, limit: usize) -> Result<Vec<(String, i64)>> {
        Ok(sqlx::query_as(
            "SELECT mac_address, updated_at FROM wol_history ORDER BY updated_at DESC LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?)
    }
}
