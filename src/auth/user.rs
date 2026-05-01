use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

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
}

impl UserStore {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
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

        let password_hash = hash_password(password)?;
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
        let user = match self.single_user().await? {
            Some(u) => u,
            None => return Ok(None),
        };

        if user.username != username {
            return Ok(None);
        }

        if verify_password(password, &user.password_hash)? {
            Ok(Some(user))
        } else {
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

        let password_hash = hash_password(new_password)?;
        let now = OffsetDateTime::now_utc();

        let result =
            sqlx::query("UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(&password_hash)
                .bind(now.format(&Rfc3339).expect("RFC3339 format"))
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

        let now = OffsetDateTime::now_utc();
        let result = sqlx::query("UPDATE users SET username = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(new_username)
            .bind(now.format(&Rfc3339).expect("RFC3339 format"))
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("User not found".to_string()));
        }

        Ok(())
    }
}
