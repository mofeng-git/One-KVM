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
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            pool,
            cache: Arc::new(ArcSwap::from_pointee(AppConfig::default())),
            change_tx: broadcast::channel(16).0,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn load(&self) -> Result<()> {
        let (mut config, removed_legacy_fields) = Self::load_config(&self.pool).await?;
        config.enforce_invariants();
        let normalized_video = config.video.normalize_sources();
        if removed_legacy_fields || normalized_video {
            Self::save_config_to_db(&self.pool, &config).await?;
        }
        self.cache.store(Arc::new(config));
        Ok(())
    }

    async fn load_config(pool: &Pool<Sqlite>) -> Result<(AppConfig, bool)> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM config WHERE key = 'app_config'")
                .fetch_optional(pool)
                .await?;

        match row {
            Some((json,)) => {
                let mut value: serde_json::Value =
                    serde_json::from_str(&json).map_err(|e| AppError::Config(e.to_string()))?;
                let mut removed = value
                    .as_object_mut()
                    .is_some_and(|object| object.remove("computer_use").is_some());
                if let Some(auth) = value
                    .get_mut("auth")
                    .and_then(|value| value.as_object_mut())
                {
                    removed |= auth.remove("totp_enabled").is_some();
                    removed |= auth.remove("totp_secret").is_some();
                }
                let config =
                    serde_json::from_value(value).map_err(|e| AppError::Config(e.to_string()))?;
                Ok((config, removed))
            }
            None => {
                let config = AppConfig::default();
                Self::save_config_to_db(pool, &config).await?;
                Ok((config, false))
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

    #[cfg(target_os = "linux")]
    pub fn hid_bonds(&self) -> crate::db::hid_bonds::HidBondStore {
        crate::db::hid_bonds::HidBondStore(self.pool.clone())
    }

    pub fn get(&self) -> Arc<AppConfig> {
        self.cache.load_full()
    }

    pub async fn set(&self, config: AppConfig) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        let mut config = config;
        config.enforce_invariants();
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
        config.enforce_invariants();

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
    async fn legacy_source_ids_are_persisted_once_without_resetting_mode() {
        let dir = tempdir().unwrap();
        let db = DatabasePool::new(&dir.path().join("sources.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let pool = db.clone_pool();
        let legacy =
            serde_json::json!({"video":{"sources":[{"device":"/dev/a"},{"device":"/dev/b"}]},
            "stream":{"mode":"webrtc"}})
            .to_string();
        sqlx::query("INSERT INTO config (key,value) VALUES ('app_config',?1)")
            .bind(legacy)
            .execute(&pool)
            .await
            .unwrap();
        let store = ConfigStore::new(pool.clone());
        store.load().await.unwrap();
        assert_eq!(store.get().video.sources[0].id, "1");
        assert_eq!(store.get().stream.mode, crate::config::StreamMode::WebRTC);
        let (saved,): (String,) = sqlx::query_as("SELECT value FROM config WHERE key='app_config'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(value["video"]["sources"][1]["id"], "2");
        let reloaded = ConfigStore::new(pool.clone());
        reloaded.load().await.unwrap();
        let (again,): (String,) = sqlx::query_as("SELECT value FROM config WHERE key='app_config'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(saved, again);
        store.update(|c| c.video.sources.reverse()).await.unwrap();
        reloaded.load().await.unwrap();
        assert_eq!(reloaded.get().video.sources[0].id, "2");
        assert_eq!(reloaded.get().video.primary_id(), "1");
        assert_eq!(
            reloaded.get().stream.mode,
            crate::config::StreamMode::WebRTC
        );
    }

    #[tokio::test]
    async fn jiggler_defaults_and_preferences_survive_restart() {
        let defaults: crate::config::HidConfig =
            serde_json::from_str(r#"{"backend":"otg"}"#).unwrap();
        assert!(!defaults.jiggler.enabled);
        assert_eq!(defaults.jiggler.interval_secs, 60);
        let dir = tempdir().unwrap();
        let db = DatabasePool::new(&dir.path().join("jiggler.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let store = ConfigStore::new(db.clone_pool());
        store.load().await.unwrap();
        store
            .update(|config| {
                config.hid.jiggler.enabled = true;
                config.hid.jiggler.interval_secs = 120;
            })
            .await
            .unwrap();
        let reloaded = ConfigStore::new(db.clone_pool());
        reloaded.load().await.unwrap();
        assert!(reloaded.get().hid.jiggler.enabled);
        assert_eq!(reloaded.get().hid.jiggler.interval_secs, 120);
    }

    #[tokio::test]
    async fn test_config_store() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let db = DatabasePool::new(&db_path).await.unwrap();
        db.init_schema().await.unwrap();

        let store = ConfigStore::new(db.clone_pool());
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

        let store2 = ConfigStore::new(db.clone_pool());
        store2.load().await.unwrap();
        let config = store2.get();
        assert!(config.initialized);
        assert_eq!(config.web.http_port, 9000);
    }

    #[tokio::test]
    async fn mcp_key_remains_readable_after_config_reload() {
        let dir = tempdir().unwrap();
        let db = DatabasePool::new(&dir.path().join("mcp-key.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let store = ConfigStore::new(db.clone_pool());
        store.load().await.unwrap();
        let key = crate::mcp::generate_key();
        store
            .update(|config| {
                config.mcp.enabled = true;
                config.mcp.api_key_hash = Some(crate::mcp::hash_key(&key));
                config.mcp.api_key = Some(key.clone());
            })
            .await
            .unwrap();
        let reloaded = ConfigStore::new(db.clone_pool());
        reloaded.load().await.unwrap();
        assert_eq!(reloaded.get().mcp.api_key.as_deref(), Some(key.as_str()));
        assert_eq!(
            reloaded.get().mcp.api_key_hash,
            store.get().mcp.api_key_hash
        );
    }

    #[tokio::test]
    async fn loading_legacy_agent_cleans_persisted_model_credentials() {
        let dir = tempdir().unwrap();
        let db = DatabasePool::new(&dir.path().join("legacy-agent.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let value =
            serde_json::json!({"computer_use":{"enabled":true,"api_key":"legacy-model-secret"}});
        sqlx::query("INSERT INTO config (key, value) VALUES ('app_config', ?1)")
            .bind(value.to_string())
            .execute(db.pool())
            .await
            .unwrap();
        let store = ConfigStore::new(db.clone_pool());
        store.load().await.unwrap();
        assert!(!store.get().mcp.enabled);
        assert!(store.get().mcp.api_key_hash.is_none());
        let (persisted,): (String,) =
            sqlx::query_as("SELECT value FROM config WHERE key = 'app_config'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(!persisted.contains("legacy-model-secret"));
        assert!(!persisted.contains("computer_use"));
    }

    #[tokio::test]
    async fn failed_watchdog_persistence_does_not_update_cache() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabasePool::new(&db_path).await.unwrap();
        db.init_schema().await.unwrap();
        let store = ConfigStore::new(db.clone_pool());
        store.load().await.unwrap();

        sqlx::query("DROP TABLE config")
            .execute(&db.clone_pool())
            .await
            .unwrap();
        assert!(store
            .update(|config| config.watchdog.enabled = true)
            .await
            .is_err());
        assert!(!store.get().watchdog.enabled);
    }

    #[tokio::test]
    async fn load_removes_legacy_totp_fields_from_persisted_config() {
        let dir = tempdir().unwrap();
        let db = DatabasePool::new(&dir.path().join("test.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        let auth = value.get_mut("auth").unwrap().as_object_mut().unwrap();
        auth.insert("totp_enabled".to_string(), serde_json::json!(true));
        auth.insert(
            "totp_secret".to_string(),
            serde_json::json!("legacy-secret"),
        );
        sqlx::query("INSERT INTO config (key, value) VALUES ('app_config', ?1)")
            .bind(value.to_string())
            .execute(db.pool())
            .await
            .unwrap();

        let store = ConfigStore::new(db.clone_pool());
        store.load().await.unwrap();
        let (persisted,): (String,) =
            sqlx::query_as("SELECT value FROM config WHERE key = 'app_config'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(!persisted.contains("totp_enabled"));
        assert!(!persisted.contains("totp_secret"));
    }
}
