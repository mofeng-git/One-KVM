use arc_swap::ArcSwap;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

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
}

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub key: String,
}

impl ConfigStore {
    /// Create a new configuration store
    pub async fn new(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        let pool = SqlitePoolOptions::new()
            // SQLite uses single-writer mode, 2 connections is sufficient for embedded devices
            // One for reads, one for writes to avoid blocking
            .max_connections(2)
            // Set reasonable timeouts for embedded environments
            .acquire_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(300))
            .connect(&db_url)
            .await?;

        // Initialize database schema
        Self::init_schema(&pool).await?;

        // Load or create default config
        let config = Self::load_config(&pool).await?;
        let cache = Arc::new(ArcSwap::from_pointee(config));

        let (change_tx, _) = broadcast::channel(16);

        Ok(Self {
            pool,
            cache,
            change_tx,
        })
    }

    /// Initialize database schema
    async fn init_schema(pool: &Pool<Sqlite>) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                is_admin INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT NOT NULL,
                data TEXT
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS api_tokens (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                permissions TEXT NOT NULL,
                expires_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_used TEXT
            )
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Load configuration from database
    async fn load_config(pool: &Pool<Sqlite>) -> Result<AppConfig> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM config WHERE key = 'app_config'"
        )
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
    /// Note: This uses a read-modify-write pattern. For concurrent updates,
    /// the last write wins. This is acceptable for configuration changes
    /// which are infrequent and typically user-initiated.
    pub async fn update<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut AppConfig),
    {
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

    /// Get database pool for session management
    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_config_store() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let store = ConfigStore::new(&db_path).await.unwrap();

        // Check default config (now lock-free, no await needed)
        let config = store.get();
        assert!(!config.initialized);

        // Update config
        store.update(|c| {
            c.initialized = true;
            c.web.http_port = 9000;
        }).await.unwrap();

        // Verify update
        let config = store.get();
        assert!(config.initialized);
        assert_eq!(config.web.http_port, 9000);

        // Create new store instance and verify persistence
        let store2 = ConfigStore::new(&db_path).await.unwrap();
        let config = store2.get();
        assert!(config.initialized);
        assert_eq!(config.web.http_port, 9000);
    }
}
