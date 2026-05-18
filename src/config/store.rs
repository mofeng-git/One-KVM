use arc_swap::ArcSwap;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

use super::AppConfig;
use super::ConfigChange;
use crate::error::{AppError, Result};

#[derive(Clone)]
pub struct ConfigStore {
    pool: Pool<Sqlite>,
    cache: Arc<ArcSwap<AppConfig>>,
    change_tx: broadcast::Sender<ConfigChange>,
    write_lock: Arc<Mutex<()>>,
}

impl ConfigStore {
    pub fn new(pool: Pool<Sqlite>) -> Result<Self> {
        Ok(Self {
            pool,
            cache: Arc::new(ArcSwap::from_pointee(AppConfig::default())),
            change_tx: broadcast::channel(16).0,
            write_lock: Arc::new(Mutex::new(())),
        })
    }

    pub async fn load(&self) -> Result<()> {
        let config = Self::load_config(&self.pool).await?;
        self.cache.store(Arc::new(config));
        Ok(())
    }

    async fn load_config(pool: &Pool<Sqlite>) -> Result<AppConfig> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM config WHERE key = 'app_config'")
                .fetch_optional(pool)
                .await?;

        match row {
            Some((json,)) => {
                serde_json::from_str(&json).map_err(|e| AppError::Config(e.to_string()))
            }
            None => {
                let config = AppConfig::default();
                Self::save_config_to_db(pool, &config).await?;
                Ok(config)
            }
        }
    }

    async fn save_config_to_db(pool: &Pool<Sqlite>, config: &AppConfig) -> Result<()> {
        let json = serde_json::to_string(config)?;

        sqlx::query(
            r#"
            INSERT INTO config (key, value, updated_at)
            VALUES ('app_config', ?1, datetime('now'))
            ON CONFLICT(key) DO UPDATE SET value = ?1, updated_at = datetime('now')
            "#,
        )
        .bind(&json)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub fn get(&self) -> Arc<AppConfig> {
        self.cache.load_full()
    }

    pub async fn set(&self, config: AppConfig) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        Self::save_config_to_db(&self.pool, &config).await?;
        self.cache.store(Arc::new(config));

        let _ = self.change_tx.send(ConfigChange {
            key: "app_config".to_string(),
        });

        Ok(())
    }

    pub async fn update<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut AppConfig),
    {
        let _guard = self.write_lock.lock().await;
        let current = self.cache.load();
        let mut config = (**current).clone();
        f(&mut config);

        Self::save_config_to_db(&self.pool, &config).await?;

        self.cache.store(Arc::new(config));

        let _ = self.change_tx.send(ConfigChange {
            key: "app_config".to_string(),
        });

        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChange> {
        self.change_tx.subscribe()
    }

    pub fn is_initialized(&self) -> bool {
        self.cache.load().initialized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DatabasePool;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_config_store() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let db = DatabasePool::new(&db_path).await.unwrap();
        db.init_schema().await.unwrap();

        let store = ConfigStore::new(db.clone_pool()).unwrap();
        store.load().await.unwrap();

        let config = store.get();
        assert!(!config.initialized);

        store
            .update(|c| {
                c.initialized = true;
                c.web.http_port = 9000;
            })
            .await
            .unwrap();

        let config = store.get();
        assert!(config.initialized);
        assert_eq!(config.web.http_port, 9000);

        let store2 = ConfigStore::new(db.clone_pool()).unwrap();
        store2.load().await.unwrap();
        let config = store2.get();
        assert!(config.initialized);
        assert_eq!(config.web.http_port, 9000);
    }
}
