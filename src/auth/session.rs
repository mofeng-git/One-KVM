use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tokio::sync::{RwLock, Notify};
use tokio_util::sync::CancellationToken;
use std::sync::atomic::{AtomicI64, Ordering};
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
    #[serde(skip)]
    cancelled: CancellationToken,
}

impl Session {
    /// Ends at the same deadline as HTTP authentication, or immediately on revocation.
    pub async fn ended(&self) {
        let remaining = (self.expires_at - OffsetDateTime::now_utc()).unsigned_abs();
        if self.is_expired() { return; }
        tokio::select! {
            biased;
            _ = self.cancelled.cancelled() => {},
            _ = tokio::time::sleep(remaining) => {},
        }
    }

    pub async fn run_until_ended<F: std::future::Future<Output = ()>>(&self, work: F) {
        tokio::select! {
            biased;
            _ = self.ended() => {},
            _ = work => {},
        }
    }

    pub fn is_expired(&self) -> bool {
        OffsetDateTime::now_utc() >= self.expires_at || self.cancelled.is_cancelled()
    }
}

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<RwLock<HashMap<String, Session>>>,
    default_ttl: Arc<AtomicI64>,
    changed: Arc<Notify>,
}

impl SessionStore {
    pub fn new(ttl_secs: i64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Arc::new(AtomicI64::new(ttl_secs.max(1))),
            changed: Arc::new(Notify::new()),
        }
    }

    /// Existing sessions retain their original deadline and browser cookie lifetime.
    pub fn set_ttl(&self, ttl_secs: i64) {
        self.default_ttl.store(ttl_secs.max(1), Ordering::Relaxed);
    }

    pub async fn create(&self, user_id: &str) -> Result<Session> {
        self.create_for_login(user_id, true).await.map(|(session, _)| session)
    }

    pub async fn create_for_login(&self, user_id: &str, allow_multiple: bool) -> Result<(Session, Vec<String>)> {
        let mut guard = self.inner.write().await;
        Self::prune(&mut guard);
        let revoked = if allow_multiple { Vec::new() } else {
            let ids = guard.keys().cloned().collect();
            for session in guard.values() { session.cancelled.cancel(); }
            guard.clear();
            ids
        };
        if guard.len() >= 256 {
            return Err(crate::error::AppError::ServiceUnavailable("Session limit reached".into()));
        }
        let now = OffsetDateTime::now_utc();
        let session = Session {
            id: Uuid::new_v4().to_string(), user_id: user_id.to_string(),
            created_at: now,
            expires_at: now + Duration::seconds(self.default_ttl.load(Ordering::Relaxed)),
            data: None, cancelled: CancellationToken::new(),
        };
        guard.insert(session.id.clone(), session.clone());
        self.changed.notify_one();
        Ok((session, revoked))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Session>> {
        Ok(self.inner.read().await.get(id).filter(|s| !s.is_expired()).cloned())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        if let Some(session) = self.inner.write().await.remove(id) { session.cancelled.cancel(); }
        self.changed.notify_one();
        Ok(())
    }

    pub async fn delete_all(&self) -> Result<u64> {
        Ok(self.delete_all_except("").await?.len() as u64)
    }

    pub async fn delete_all_except(&self, id: &str) -> Result<Vec<String>> {
        let mut guard = self.inner.write().await;
        let mut revoked = Vec::new();
        guard.retain(|key, session| {
            if key == id { return true; }
            session.cancelled.cancel();
            revoked.push(key.clone());
            false
        });
        self.changed.notify_one();
        Ok(revoked)
    }

    pub async fn list_ids(&self) -> Result<Vec<String>> {
        Ok(self.inner.read().await.iter().filter(|(_, s)| !s.is_expired()).map(|(id, _)| id.clone()).collect())
    }

    fn prune(sessions: &mut HashMap<String, Session>) {
        sessions.retain(|_, session| {
            if session.is_expired() { session.cancelled.cancel(); false } else { true }
        });
    }

    /// One deadline-driven reaper per authentication realm; no per-client polling.
    pub async fn reap(&self) {
        loop {
            let changed = self.changed.notified();
            let next = {
                let mut guard = self.inner.write().await;
                Self::prune(&mut guard);
                guard.values().map(|s| s.expires_at).min()
            };
            match next {
                Some(deadline) => {
                    let delay = (deadline - OffsetDateTime::now_utc()).max(Duration::ZERO).unsigned_abs();
                    tokio::select! { _ = changed => {}, _ = tokio::time::sleep(delay) => {} }
                }
                None => changed.await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn delete_all_except_preserves_only_current_session() {
        let sessions = SessionStore::new(60);
        let current = sessions.create("user").await.unwrap();
        let other = sessions.create("user").await.unwrap();

        let revoked = sessions.delete_all_except(&current.id).await.unwrap();
        assert_eq!(revoked, vec![other.id.clone()]);
        assert!(sessions.get(&current.id).await.unwrap().is_some());
        assert!(sessions.get(&other.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn login_creation_applies_session_policy_atomically() {
        let sessions = SessionStore::new(60);
        let existing = sessions.create("user").await.unwrap();

        let (multiple, revoked) = sessions.create_for_login("user", true).await.unwrap();
        assert!(revoked.is_empty());
        assert!(sessions.get(&existing.id).await.unwrap().is_some());
        assert!(sessions.get(&multiple.id).await.unwrap().is_some());

        let (single, mut revoked) = sessions.create_for_login("user", false).await.unwrap();
        revoked.sort();
        let mut expected = vec![existing.id, multiple.id];
        expected.sort();
        assert_eq!(revoked, expected);
        assert_eq!(sessions.list_ids().await.unwrap(), vec![single.id]);
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use std::time::Duration as StdDuration;

    async fn assert_ended(session: &Session) {
        tokio::time::timeout(StdDuration::from_millis(100), session.ended()).await.unwrap();
    }

    #[tokio::test]
    async fn logout_single_login_and_account_revocation_signal_connections() {
        let store = SessionStore::new(60);
        let logout = store.create("user").await.unwrap();
        store.delete(&logout.id).await.unwrap();
        assert_ended(&logout).await;
        let previous = store.create("user").await.unwrap();
        let (current, _) = store.create_for_login("user", false).await.unwrap();
        assert_ended(&previous).await;
        let other = store.create("user").await.unwrap();
        // Shared operation used by password and TOTP changes.
        store.delete_all_except(&current.id).await.unwrap();
        assert_ended(&other).await;
        assert!(!current.is_expired());
    }

    #[tokio::test]
    async fn expiration_ends_connections_and_reclaims_unvisited_sessions() {
        let store = SessionStore::new(60);
        let session = store.create("user").await.unwrap();
        store.inner.write().await.get_mut(&session.id).unwrap().expires_at = OffsetDateTime::now_utc() + Duration::milliseconds(20);
        let session = store.get(&session.id).await.unwrap().unwrap();
        let task_store = store.clone();
        let task = tokio::spawn(async move { task_store.reap().await });
        assert_ended(&session).await;
        tokio::time::sleep(StdDuration::from_millis(20)).await;
        assert!(store.inner.read().await.is_empty());
        task.abort();
    }

    #[tokio::test]
    async fn ttl_changes_only_new_sessions_and_realms_are_isolated() {
        let web = SessionStore::new(60);
        let redfish = SessionStore::new(60);
        let old = web.create("user").await.unwrap();
        let rf = redfish.create("user").await.unwrap();
        assert!(web.get(&rf.id).await.unwrap().is_none());
        assert!(redfish.get(&old.id).await.unwrap().is_none());
        web.set_ttl(120);
        let new = web.create("user").await.unwrap();
        assert_eq!((new.expires_at-new.created_at).whole_seconds(),120);
        assert_eq!((old.expires_at-old.created_at).whole_seconds(),60);
        web.delete_all().await.unwrap();
        assert!(!rf.is_expired());
    }

    #[tokio::test]
    async fn revoked_session_cancels_blocked_io_and_drops_its_resources() {
        let store = SessionStore::new(60);
        let session = store.create("user").await.unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move { session.run_until_ended(async move { let _tx = tx; std::future::pending::<()>().await }).await });
        store.delete_all().await.unwrap();
        tokio::time::timeout(StdDuration::from_millis(100), task).await.unwrap().unwrap();
        assert!(rx.await.is_err());
    }

    #[tokio::test]
    async fn session_capacity_is_bounded() {
        let store = SessionStore::new(60);
        for _ in 0..256 { store.create("user").await.unwrap(); }
        assert!(store.create("user").await.is_err());
        assert!(store.create_for_login("user", false).await.is_ok());
    }
}
