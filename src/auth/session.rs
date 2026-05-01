use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    pub data: Option<serde_json::Value>,
}

impl Session {
    pub fn is_expired(&self) -> bool {
        OffsetDateTime::now_utc() > self.expires_at
    }
}

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<RwLock<HashMap<String, Session>>>,
    default_ttl: Duration,
}

impl SessionStore {
    pub fn new(ttl_secs: i64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::seconds(ttl_secs),
        }
    }

    pub async fn create(&self, user_id: &str) -> Result<Session> {
        let now = OffsetDateTime::now_utc();
        let session = Session {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            created_at: now,
            expires_at: now + self.default_ttl,
            data: None,
        };

        let mut guard = self.inner.write().await;
        guard.insert(session.id.clone(), session.clone());
        Ok(session)
    }

    pub async fn get(&self, session_id: &str) -> Result<Option<Session>> {
        let mut guard = self.inner.write().await;
        let Some(session) = guard.get(session_id).cloned() else {
            return Ok(None);
        };
        if session.is_expired() {
            guard.remove(session_id);
            return Ok(None);
        }
        Ok(Some(session))
    }

    pub async fn delete(&self, session_id: &str) -> Result<()> {
        let mut guard = self.inner.write().await;
        guard.remove(session_id);
        Ok(())
    }

    pub async fn cleanup_expired(&self) -> Result<u64> {
        let mut guard = self.inner.write().await;
        let before = guard.len();
        guard.retain(|_, s| !s.is_expired());
        Ok((before - guard.len()) as u64)
    }

    pub async fn delete_all(&self) -> Result<u64> {
        let mut guard = self.inner.write().await;
        let n = guard.len() as u64;
        guard.clear();
        Ok(n)
    }

    pub async fn list_ids(&self) -> Result<Vec<String>> {
        let guard = self.inner.read().await;
        Ok(guard.keys().cloned().collect())
    }

    pub async fn extend(&self, session_id: &str) -> Result<()> {
        let mut guard = self.inner.write().await;
        if let Some(session) = guard.get_mut(session_id) {
            if session.is_expired() {
                guard.remove(session_id);
            } else {
                session.expires_at = OffsetDateTime::now_utc() + self.default_ttl;
            }
        }
        Ok(())
    }
}
