use arc_swap::ArcSwap;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

use super::persistence::ConfigChange;
use super::AppConfig;
use crate::error::{AppError, Result};

/// Configuration store backed by SQLite
///
/// Uses `ArcSwap` for lock-free reads, providing high performance
/// for frequent configuration access in hot paths.
#[derive(Clone)]
pub struct ConfigStore {
    pool: Pool<Sqlite>,
    /// Lock-free cache using ArcSwap for zero-cost reads
    cache: Arc<ArcSwap<AppConfig>>,
    change_tx: broadcast::Sender<ConfigChange>,
    /// Serializes `set` / `update` so concurrent PATCH handlers cannot clobber each other
    write_lock: Arc<Mutex<()>>,
}

impl ConfigStore {
    /// Create a new configuration store
    pub fn new(pool: Pool<Sqlite>) -> Result<Self> {
        // Load or create default config synchronously wrapper
        // (actual DB load is async, handled in init())
        Ok(Self {
            pool,
            cache: Arc::new(ArcSwap::from_pointee(AppConfig::default())),
            change_tx: broadcast::channel(16).0,
            write_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Load configuration from database (call after new())
    pub async fn load(&self) -> Result<()> {
        let config = Self::load_config(&self.pool).await?;
        self.cache.store(Arc::new(config));
        Ok(())
    }

    /// Load configuration from database
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
                // Create default config
                let config = AppConfig::default();
                Self::save_config_to_db(pool, &config).await?;
                Ok(config)
            }
        }
    }

    /// Save configuration to database
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

    /// Get current configuration (lock-free, zero-copy)
    ///
    /// Returns an `Arc<AppConfig>` for efficient sharing without cloning.
    /// This is a lock-free operation with minimal overhead.
    pub fn get(&self) -> Arc<AppConfig> {
        self.cache.load_full()
    }

    /// Set entire configuration
    pub async fn set(&self, config: AppConfig) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        Self::save_config_to_db(&self.pool, &config).await?;
        self.cache.store(Arc::new(config));

        // Notify subscribers
        let _ = self.change_tx.send(ConfigChange {
            key: "app_config".to_string(),
        });

        Ok(())
    }

    /// Update configuration with a closure
    ///
    /// Uses read-modify-write under a mutex so concurrent `update` / `set` calls are serialized
    /// and merged correctly (each closure sees the latest stored config).
    pub async fn update<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut AppConfig),
    {
        let _guard = self.write_lock.lock().await;
        // Load current config, clone it for modification
        let current = self.cache.load();
        let mut config = (**current).clone();
        f(&mut config);

        // Persist to database first
        Self::save_config_to_db(&self.pool, &config).await?;

        // Then update cache atomically
        self.cache.store(Arc::new(config));

        // Notify subscribers
        let _ = self.change_tx.send(ConfigChange {
            key: "app_config".to_string(),
        });

        Ok(())
    }

    /// Subscribe to configuration changes
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChange> {
        self.change_tx.subscribe()
    }

    /// Check if system is initialized (lock-free)
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

        // Check default config (now lock-free, no await needed)
        let config = store.get();
        assert!(!config.initialized);

        // Update config
        store
            .update(|c| {
                c.initialized = true;
                c.web.http_port = 9000;
            })
            .await
            .unwrap();

        // Verify update
        let config = store.get();
        assert!(config.initialized);
        assert_eq!(config.web.http_port, 9000);

        // Create new store instance and verify persistence
        let store2 = ConfigStore::new(db.clone_pool()).unwrap();
        store2.load().await.unwrap();
        let config = store2.get();
        assert!(config.initialized);
        assert_eq!(config.web.http_port, 9000);
    }
}
