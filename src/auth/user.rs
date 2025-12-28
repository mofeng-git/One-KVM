use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use crate::error::{AppError, Result};
use super::password::{hash_password, verify_password};

/// User row type from database
type UserRow = (String, String, String, i32, String, String);

/// User data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Convert from database row to User
    fn from_row(row: UserRow) -> Self {
        let (id, username, password_hash, is_admin, created_at, updated_at) = row;
        Self {
            id,
            username,
            password_hash,
            is_admin: is_admin != 0,
            created_at: DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }
    }
}

/// User store backed by SQLite
#[derive(Clone)]
pub struct UserStore {
    pool: Pool<Sqlite>,
}

impl UserStore {
    /// Create a new user store
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Create a new user
    pub async fn create(&self, username: &str, password: &str, is_admin: bool) -> Result<User> {
        // Check if username already exists
        if self.get_by_username(username).await?.is_some() {
            return Err(AppError::BadRequest(format!(
                "Username '{}' already exists",
                username
            )));
        }

        let password_hash = hash_password(password)?;
        let now = Utc::now();
        let user = User {
            id: Uuid::new_v4().to_string(),
            username: username.to_string(),
            password_hash,
            is_admin,
            created_at: now,
            updated_at: now,
        };

        sqlx::query(
            r#"
            INSERT INTO users (id, username, password_hash, is_admin, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&user.id)
        .bind(&user.username)
        .bind(&user.password_hash)
        .bind(user.is_admin as i32)
        .bind(user.created_at.to_rfc3339())
        .bind(user.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(user)
    }

    /// Get user by ID
    pub async fn get(&self, user_id: &str) -> Result<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, is_admin, created_at, updated_at FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(User::from_row))
    }

    /// Get user by username
    pub async fn get_by_username(&self, username: &str) -> Result<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, is_admin, created_at, updated_at FROM users WHERE username = ?1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(User::from_row))
    }

    /// Verify user credentials
    pub async fn verify(&self, username: &str, password: &str) -> Result<Option<User>> {
        let user = match self.get_by_username(username).await? {
            Some(user) => user,
            None => return Ok(None),
        };

        if verify_password(password, &user.password_hash)? {
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    /// Update user password
    pub async fn update_password(&self, user_id: &str, new_password: &str) -> Result<()> {
        let password_hash = hash_password(new_password)?;
        let now = Utc::now();

        let result = sqlx::query(
            "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
        )
        .bind(&password_hash)
        .bind(now.to_rfc3339())
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("User not found".to_string()));
        }

        Ok(())
    }

    /// List all users
    pub async fn list(&self) -> Result<Vec<User>> {
        let rows: Vec<UserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, is_admin, created_at, updated_at FROM users ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(User::from_row).collect())
    }

    /// Delete user by ID
    pub async fn delete(&self, user_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("User not found".to_string()));
        }

        Ok(())
    }

    /// Check if any users exist
    pub async fn has_users(&self) -> Result<bool> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0 > 0)
    }
}
