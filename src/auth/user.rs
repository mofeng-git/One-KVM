use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;
use std::{sync::Arc, collections::VecDeque, time::{Duration, Instant}};
use tokio::sync::Semaphore;
use parking_lot::Mutex;

#[derive(Default)]
struct PasswordFailures(Mutex<VecDeque<Instant>>);
impl PasswordFailures {
    fn check(&self) -> Result<()> {
        let mut failures = self.0.lock();
        let now = Instant::now();
        failures.retain(|at| now.duration_since(*at) < Duration::from_secs(60));
        if failures.len() >= 8 {
            return Err(AppError::ServiceUnavailable("Password attempts temporarily limited".into()));
        }
        Ok(())
    }
    fn record(&self) { let mut failures = self.0.lock(); if failures.len() < 8 { failures.push_back(Instant::now()); } }
}


use super::password::{hash_password, verify_password};
use crate::error::{AppError, Result};

type UserRow = (String, String, String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
}

impl User {
    fn from_row(row: UserRow) -> Self {
        let (id, username, password_hash) = row;
        Self {
            id,
            username,
            password_hash,
        }
    }
}

#[derive(Clone)]
pub struct UserStore {
    pool: Pool<Sqlite>,
    password_workers: Arc<Semaphore>,
    failures: Arc<PasswordFailures>,
}

impl UserStore {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool, password_workers: Arc::new(Semaphore::new(2)), failures: Arc::new(PasswordFailures::default()) }
    }

    async fn password_work<T: Send + 'static>(&self, work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
        // Admission occurs before spawning: no unbounded waiting queue or blocking tasks.
        let permit = self.password_workers.clone().try_acquire_owned()
            .map_err(|_| AppError::ServiceUnavailable("Password service busy".into()))?;
        tokio::task::spawn_blocking(move || { let _permit = permit; work() }).await
            .map_err(|e| AppError::Internal(format!("Password worker failed: {e}")))?
    }

    async fn hash(&self, password: &str) -> Result<String> {
        let password = password.to_owned();
        self.password_work(move || hash_password(&password)).await
    }

    /// The single local user, or `None` if none exists. Errors if more than one row is present.
    pub async fn single_user(&self) -> Result<Option<User>> {
        let mut rows: Vec<UserRow> = sqlx::query_as(
            "SELECT id, username, password_hash FROM users ORDER BY rowid ASC LIMIT 2",
        )
        .fetch_all(&self.pool)
        .await?;

        match rows.len() {
            0 => Ok(None),
            1 => Ok(Some(User::from_row(rows.remove(0)))),
            _ => Err(AppError::Internal(
                "Multiple user accounts in database; this build supports only one".to_string(),
            )),
        }
    }

    pub async fn create_first_user(&self, username: &str, password: &str) -> Result<User> {
        if self.single_user().await?.is_some() {
            return Err(AppError::BadRequest(
                "A user account already exists".to_string(),
            ));
        }

        let password_hash = self.hash(password).await?;
        let user = User {
            id: Uuid::new_v4().to_string(),
            username: username.to_string(),
            password_hash,
        };

        sqlx::query(
            r#"
            INSERT INTO users (id, username, password_hash)
            VALUES (?1, ?2, ?3)
            "#,
        )
        .bind(&user.id)
        .bind(&user.username)
        .bind(&user.password_hash)
        .execute(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn verify(&self, username: &str, password: &str) -> Result<Option<User>> {
        self.failures.check()?;
        let user = match self.single_user().await? {
            Some(u) => u,
            None => return Ok(None),
        };

        if user.username != username {
            self.failures.record();
            return Ok(None);
        }

        let password = password.to_owned();
        let hash = user.password_hash.clone();
        if self.password_work(move || verify_password(&password, &hash)).await? {
            Ok(Some(user))
        } else {
            self.failures.record();
            Ok(None)
        }
    }

    pub async fn update_password(&self, user_id: &str, new_password: &str) -> Result<()> {
        let user = self
            .single_user()
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        if user.id != user_id {
            return Err(AppError::AuthError("Invalid session".to_string()));
        }

        let password_hash = self.hash(new_password).await?;
        let result = sqlx::query(
            "UPDATE users SET password_hash = ?1, updated_at = datetime('now') WHERE id = ?2",
        )
        .bind(&password_hash)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("User not found".to_string()));
        }

        Ok(())
    }

    pub async fn update_username(&self, user_id: &str, new_username: &str) -> Result<()> {
        let user = self
            .single_user()
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        if user.id != user_id {
            return Err(AppError::AuthError("Invalid session".to_string()));
        }

        if new_username == user.username {
            return Ok(());
        }

        let result = sqlx::query(
            "UPDATE users SET username = ?1, updated_at = datetime('now') WHERE id = ?2",
        )
        .bind(new_username)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("User not found".to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod password_worker_tests {
    use super::*;

    #[test]
    fn failed_attempt_window_is_bounded_and_expires() {
        let failures = PasswordFailures::default();
        for _ in 0..100 { failures.record(); }
        assert_eq!(failures.0.lock().len(), 8);
        assert!(failures.check().is_err());
        *failures.0.lock() = VecDeque::from([Instant::now() - Duration::from_secs(61)]);
        assert!(failures.check().is_ok());
    }

    #[tokio::test]
    async fn saturated_password_workers_reject_without_spawning() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().connect_lazy("sqlite::memory:").unwrap();
        let users = UserStore::new(pool);
        let permits = users.password_workers.acquire_many(2).await.unwrap();
        assert!(users.password_work(|| -> Result<()> { panic!("must not spawn") }).await.is_err());
        drop(permits);
        assert_eq!(users.password_work(|| Ok(42)).await.unwrap(), 42);
    }
}
