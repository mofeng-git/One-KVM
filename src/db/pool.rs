use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Pool, Sqlite,
};
use std::path::Path;
use std::time::Duration;

use crate::error::Result;

#[derive(Clone)]
pub struct DatabasePool {
    pool: Pool<Sqlite>,
}

impl DatabasePool {
    pub async fn new(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(300))
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    pub async fn init_schema(&self) -> Result<()> {
        // Keep migrations embedded in the binary so deployments do not need an
        // extra migrations directory or another runtime dependency.
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&mut *transaction)
        .await?;

        let current_version: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM schema_migrations")
                .fetch_one(&mut *transaction)
                .await?;

        for (version, statements) in SCHEMA_MIGRATIONS
            .iter()
            .enumerate()
            .map(|(index, statements)| ((index + 1) as i64, *statements))
        {
            if version <= current_version {
                continue;
            }
            for &statement in statements {
                sqlx::query(statement).execute(&mut *transaction).await?;
            }
            sqlx::query("INSERT INTO schema_migrations (version) VALUES (?1)")
                .bind(version)
                .execute(&mut *transaction)
                .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    pub fn clone_pool(&self) -> Pool<Sqlite> {
        self.pool.clone()
    }

    pub fn wol_history(&self) -> super::WolHistoryStore {
        super::WolHistoryStore::new(self.pool.clone())
    }
}

// Each item is one version; statements within a version run atomically.
// New schema changes should be appended as a new item, never edited in place.
const SCHEMA_MIGRATIONS: &[&[&str]] = &[
    &[
        r#"
    CREATE TABLE IF NOT EXISTS config (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    )
    "#,
        r#"
    CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY,
        username TEXT NOT NULL UNIQUE,
        password_hash TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    )
    "#,
        r#"
    CREATE TABLE IF NOT EXISTS user_totp_credentials (
        user_id TEXT PRIMARY KEY,
        secret TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
    )
    "#,
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
        r#"
    CREATE TABLE IF NOT EXISTS wol_history (
        mac_address TEXT PRIMARY KEY,
        updated_at INTEGER NOT NULL
    )
    "#,
        r#"
    CREATE INDEX IF NOT EXISTS idx_wol_history_updated_at
    ON wol_history(updated_at DESC)
    "#,
    ],
    &[r#"
    CREATE UNIQUE INDEX IF NOT EXISTS idx_users_singleton
    ON users ((1))
    "#],
    &["CREATE TABLE hid_bonds (adapter TEXT NOT NULL, peer TEXT NOT NULL, pending INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(adapter, peer))"],
];
