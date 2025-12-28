use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use crate::error::Result;

/// Session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub data: Option<serde_json::Value>,
}

impl Session {
    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Session store backed by SQLite
#[derive(Clone)]
pub struct SessionStore {
    pool: Pool<Sqlite>,
    default_ttl: Duration,
}

impl SessionStore {
    /// Create a new session store
    pub fn new(pool: Pool<Sqlite>, ttl_secs: i64) -> Self {
        Self {
            pool,
            default_ttl: Duration::seconds(ttl_secs),
        }
    }

    /// Create a new session
    pub async fn create(&self, user_id: &str) -> Result<Session> {
        let session = Session {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + self.default_ttl,
            data: None,
        };

        sqlx::query(
            r#"
            INSERT INTO sessions (id, user_id, created_at, expires_at, data)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(&session.id)
        .bind(&session.user_id)
        .bind(session.created_at.to_rfc3339())
        .bind(session.expires_at.to_rfc3339())
        .bind(session.data.as_ref().map(|d| d.to_string()))
        .execute(&self.pool)
        .await?;

        Ok(session)
    }

    /// Get a session by ID
    pub async fn get(&self, session_id: &str) -> Result<Option<Session>> {
        let row: Option<(String, String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, user_id, created_at, expires_at, data FROM sessions WHERE id = ?1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((id, user_id, created_at, expires_at, data)) => {
                let session = Session {
                    id,
                    user_id,
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    expires_at: DateTime::parse_from_rfc3339(&expires_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    data: data.and_then(|d| serde_json::from_str(&d).ok()),
                };

                if session.is_expired() {
                    self.delete(&session.id).await?;
                    Ok(None)
                } else {
                    Ok(Some(session))
                }
            }
            None => Ok(None),
        }
    }

    /// Delete a session
    pub async fn delete(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete all expired sessions
    pub async fn cleanup_expired(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < datetime('now')")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Extend session expiration
    pub async fn extend(&self, session_id: &str) -> Result<()> {
        let new_expires = Utc::now() + self.default_ttl;
        sqlx::query("UPDATE sessions SET expires_at = ?1 WHERE id = ?2")
            .bind(new_expires.to_rfc3339())
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
