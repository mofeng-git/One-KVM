use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sqlx::{Pool, Sqlite};
use tokio::sync::Mutex;
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

use crate::error::{AppError, Result};

const LOGIN_TTL: Duration = Duration::from_secs(5 * 60);
const ENROLLMENT_TTL: Duration = Duration::from_secs(10 * 60);
const FAILURE_WINDOW: Duration = Duration::from_secs(60);
const MAX_FAILURES: usize = 5;

struct LoginChallenge {
    id: String,
    user_id: String,
    expires_at: Instant,
    expires_at_unix_ms: u64,
    failures: usize,
}

struct EnrollmentChallenge {
    id: String,
    user_id: String,
    secret: Secret,
    expires_at: Instant,
    expires_at_unix_ms: u64,
    failures: usize,
}

#[derive(Clone)]
pub struct ChallengeInfo {
    pub id: String,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone)]
pub struct EnrollmentInfo {
    pub id: String,
    pub secret: String,
    pub otpauth_uri: String,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone)]
pub struct TwoFactorService {
    pool: Pool<Sqlite>,
    login_challenges: std::sync::Arc<Mutex<HashMap<String, LoginChallenge>>>,
    enrollment_challenges: std::sync::Arc<Mutex<HashMap<String, EnrollmentChallenge>>>,
    failures: std::sync::Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

impl TwoFactorService {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            pool,
            login_challenges: Default::default(),
            enrollment_challenges: Default::default(),
            failures: Default::default(),
        }
    }

    pub async fn is_enabled(&self, user_id: &str) -> Result<bool> {
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM user_totp_credentials WHERE user_id = ?1 LIMIT 1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(exists.is_some())
    }

    pub async fn begin_login(&self, user_id: &str) -> Result<Option<ChallengeInfo>> {
        if !self.is_enabled(user_id).await? {
            return Ok(None);
        }

        let challenge = LoginChallenge {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            expires_at: Instant::now() + LOGIN_TTL,
            expires_at_unix_ms: expires_at_unix_ms(LOGIN_TTL),
            failures: 0,
        };
        let info = ChallengeInfo {
            id: challenge.id.clone(),
            expires_at_unix_ms: challenge.expires_at_unix_ms,
        };
        self.login_challenges
            .lock()
            .await
            .insert(user_id.to_string(), challenge);
        Ok(Some(info))
    }

    pub async fn complete_login(&self, challenge_id: &str, code: &str) -> Result<String> {
        validate_code_format(code)?;

        let (user_id, expired) = {
            let challenges = self.login_challenges.lock().await;
            let challenge = challenges
                .values()
                .find(|challenge| challenge.id == challenge_id)
                .ok_or_else(|| AppError::AuthError("TOTP challenge expired".to_string()))?;
            (
                challenge.user_id.clone(),
                Instant::now() >= challenge.expires_at,
            )
        };

        if expired {
            self.login_challenges.lock().await.remove(&user_id);
            return Err(AppError::AuthError("TOTP challenge expired".to_string()));
        }
        self.enforce_failure_limit(&user_id).await?;

        let valid = match self.credential_secret(&user_id).await? {
            Some(secret) => verify_at(&secret, code, unix_time_secs())?,
            None => {
                self.login_challenges.lock().await.remove(&user_id);
                return Err(AppError::AuthError("TOTP challenge expired".to_string()));
            }
        };
        if !valid {
            self.record_failure(&user_id).await;
            let mut challenges = self.login_challenges.lock().await;
            let mut exhausted = false;
            if let Some(challenge) = challenges.get_mut(&user_id) {
                challenge.failures += 1;
                if challenge.failures >= MAX_FAILURES {
                    exhausted = true;
                    challenges.remove(&user_id);
                }
            }
            if exhausted {
                return Err(AppError::AuthError("TOTP challenge expired".to_string()));
            }
            return Err(AppError::AuthError("Invalid TOTP code".to_string()));
        }

        let consumed = self
            .login_challenges
            .lock()
            .await
            .remove(&user_id)
            .is_some_and(|challenge| challenge.id == challenge_id);
        if !consumed {
            return Err(AppError::AuthError("TOTP challenge expired".to_string()));
        }
        Ok(user_id)
    }

    pub async fn begin_enrollment(
        &self,
        session_id: &str,
        user_id: &str,
        username: &str,
    ) -> Result<EnrollmentInfo> {
        if self.is_enabled(user_id).await? {
            return Err(AppError::Conflict("TOTP is already enabled".to_string()));
        }

        let secret = Secret::generate_secret().to_encoded();
        let totp = totp(&secret, username)?;
        let challenge = EnrollmentChallenge {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            secret,
            expires_at: Instant::now() + ENROLLMENT_TTL,
            expires_at_unix_ms: expires_at_unix_ms(ENROLLMENT_TTL),
            failures: 0,
        };
        let info = EnrollmentInfo {
            id: challenge.id.clone(),
            secret: challenge.secret.to_string(),
            otpauth_uri: totp.get_url(),
            expires_at_unix_ms: challenge.expires_at_unix_ms,
        };
        self.enrollment_challenges
            .lock()
            .await
            .insert(session_id.to_string(), challenge);
        Ok(info)
    }

    pub async fn confirm_enrollment(
        &self,
        session_id: &str,
        user_id: &str,
        enrollment_id: &str,
        code: &str,
    ) -> Result<()> {
        validate_code_format(code)?;
        self.enforce_failure_limit(user_id).await?;

        let (secret, expired) = {
            let challenges = self.enrollment_challenges.lock().await;
            let challenge = challenges
                .get(session_id)
                .filter(|challenge| challenge.id == enrollment_id && challenge.user_id == user_id)
                .ok_or_else(|| AppError::AuthError("TOTP enrollment expired".to_string()))?;
            (
                challenge.secret.clone(),
                Instant::now() >= challenge.expires_at,
            )
        };
        if expired {
            self.enrollment_challenges.lock().await.remove(session_id);
            return Err(AppError::AuthError("TOTP enrollment expired".to_string()));
        }

        if !verify_at(&secret, code, unix_time_secs())? {
            self.record_failure(user_id).await;
            let mut challenges = self.enrollment_challenges.lock().await;
            let mut exhausted = false;
            if let Some(challenge) = challenges.get_mut(session_id) {
                challenge.failures += 1;
                if challenge.failures >= MAX_FAILURES {
                    exhausted = true;
                    challenges.remove(session_id);
                }
            }
            if exhausted {
                return Err(AppError::AuthError("TOTP enrollment expired".to_string()));
            }
            return Err(AppError::AuthError("Invalid TOTP code".to_string()));
        }

        let mut transaction = self.pool.begin().await?;
        let result =
            sqlx::query("INSERT INTO user_totp_credentials (user_id, secret) VALUES (?1, ?2)")
                .bind(user_id)
                .bind(secret.to_string())
                .execute(&mut *transaction)
                .await;
        match result {
            Ok(_) => transaction.commit().await?,
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                return Err(AppError::Conflict("TOTP is already enabled".to_string()));
            }
            Err(error) => return Err(error.into()),
        }
        self.enrollment_challenges.lock().await.remove(session_id);
        Ok(())
    }

    pub async fn disable(&self, user_id: &str, code: &str) -> Result<()> {
        validate_code_format(code)?;
        self.enforce_failure_limit(user_id).await?;
        let secret = self
            .credential_secret(user_id)
            .await?
            .ok_or_else(|| AppError::Conflict("TOTP is not enabled".to_string()))?;
        if !verify_at(&secret, code, unix_time_secs())? {
            self.record_failure(user_id).await;
            return Err(AppError::AuthError("Invalid TOTP code".to_string()));
        }

        let result = sqlx::query("DELETE FROM user_totp_credentials WHERE user_id = ?1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Conflict("TOTP is not enabled".to_string()));
        }
        self.clear_user_challenges(user_id).await;
        Ok(())
    }

    pub async fn disable_without_code(&self, user_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM user_totp_credentials WHERE user_id = ?1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        self.clear_user_challenges(user_id).await;
        Ok(result.rows_affected() > 0)
    }

    async fn credential_secret(&self, user_id: &str) -> Result<Option<Secret>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT secret FROM user_totp_credentials WHERE user_id = ?1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(secret,)| Secret::Encoded(secret)))
    }

    async fn enforce_failure_limit(&self, user_id: &str) -> Result<()> {
        let now = Instant::now();
        let mut failures = self.failures.lock().await;
        let attempts = failures.entry(user_id.to_string()).or_default();
        while attempts
            .front()
            .is_some_and(|attempt| now.duration_since(*attempt) >= FAILURE_WINDOW)
        {
            attempts.pop_front();
        }
        if attempts.len() >= MAX_FAILURES {
            return Err(AppError::RateLimited(
                "TOTP verification is temporarily limited".to_string(),
            ));
        }
        Ok(())
    }

    async fn record_failure(&self, user_id: &str) {
        self.failures
            .lock()
            .await
            .entry(user_id.to_string())
            .or_default()
            .push_back(Instant::now());
    }

    async fn clear_user_challenges(&self, user_id: &str) {
        self.login_challenges.lock().await.remove(user_id);
        self.enrollment_challenges
            .lock()
            .await
            .retain(|_, challenge| challenge.user_id != user_id);
        self.failures.lock().await.remove(user_id);
    }
}

fn totp(secret: &Secret, account_name: &str) -> Result<TOTP> {
    let account_name = account_name.replace(':', "_");
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret
            .to_bytes()
            .map_err(|error| AppError::Internal(error.to_string()))?,
        Some("One-KVM".to_string()),
        account_name,
    )
    .map_err(|error| AppError::Internal(error.to_string()))
}

fn verify_at(secret: &Secret, code: &str, unix_time: u64) -> Result<bool> {
    validate_code_format(code)?;
    Ok(totp(secret, "user")?.check(code, unix_time))
}

fn validate_code_format(code: &str) -> Result<()> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "TOTP code must contain exactly 6 digits".to_string(),
        ));
    }
    Ok(())
}

pub fn server_time_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn unix_time_secs() -> u64 {
    server_time_unix_ms() / 1000
}

fn expires_at_unix_ms(ttl: Duration) -> u64 {
    server_time_unix_ms().saturating_add(ttl.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DatabasePool;
    use tempfile::tempdir;

    async fn test_service() -> (tempfile::TempDir, TwoFactorService, String) {
        let dir = tempdir().unwrap();
        let db = DatabasePool::new(&dir.path().join("test.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let user_id = "user-1".to_string();
        sqlx::query("INSERT INTO users (id, username, password_hash) VALUES (?1, 'admin', 'hash')")
            .bind(&user_id)
            .execute(db.pool())
            .await
            .unwrap();
        let service = TwoFactorService::new(db.clone_pool());
        (dir, service, user_id)
    }

    async fn install_known_credential(service: &TwoFactorService, user_id: &str) -> Secret {
        let secret = Secret::Raw(b"12345678901234567890".to_vec()).to_encoded();
        sqlx::query("INSERT INTO user_totp_credentials (user_id, secret) VALUES (?1, ?2)")
            .bind(user_id)
            .bind(secret.to_string())
            .execute(&service.pool)
            .await
            .unwrap();
        secret
    }

    #[test]
    fn accepts_rfc_vector_and_adjacent_window() {
        let secret = Secret::Raw(b"12345678901234567890".to_vec());
        assert!(verify_at(&secret, "287082", 59).unwrap());
        let code = totp(&secret, "user").unwrap().generate(30);
        assert!(verify_at(&secret, &code, 60).unwrap());
    }

    #[test]
    fn rejects_malformed_codes() {
        let secret = Secret::Raw(b"12345678901234567890".to_vec());
        assert!(verify_at(&secret, "12345", 59).is_err());
        assert!(verify_at(&secret, "12345x", 59).is_err());
    }

    #[tokio::test]
    async fn login_challenges_are_replaced_expire_and_are_consumed_once() {
        let (_dir, service, user_id) = test_service().await;
        let secret = install_known_credential(&service, &user_id).await;
        let first = service.begin_login(&user_id).await.unwrap().unwrap();
        let second = service.begin_login(&user_id).await.unwrap().unwrap();
        let code = totp(&secret, "user").unwrap().generate_current().unwrap();

        assert!(service.complete_login(&first.id, &code).await.is_err());
        assert_eq!(
            service.complete_login(&second.id, &code).await.unwrap(),
            user_id
        );
        assert!(service.complete_login(&second.id, &code).await.is_err());

        let expired = service.begin_login(&user_id).await.unwrap().unwrap();
        service
            .login_challenges
            .lock()
            .await
            .get_mut(&user_id)
            .unwrap()
            .expires_at = Instant::now() - Duration::from_secs(1);
        assert!(service.complete_login(&expired.id, &code).await.is_err());
    }

    #[tokio::test]
    async fn challenge_failure_limit_is_shared_across_new_challenges() {
        let (_dir, service, user_id) = test_service().await;
        let secret = install_known_credential(&service, &user_id).await;
        let valid = totp(&secret, "user").unwrap().generate_current().unwrap();
        let invalid = if valid == "000000" {
            "000001"
        } else {
            "000000"
        };
        let challenge = service.begin_login(&user_id).await.unwrap().unwrap();

        for _ in 0..4 {
            let error = service
                .complete_login(&challenge.id, invalid)
                .await
                .unwrap_err();
            assert!(matches!(error, AppError::AuthError(_)));
        }
        let error = service
            .complete_login(&challenge.id, invalid)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("challenge expired"));

        let replacement = service.begin_login(&user_id).await.unwrap().unwrap();
        let error = service
            .complete_login(&replacement.id, &valid)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::RateLimited(_)));
    }

    #[tokio::test]
    async fn enrollment_persists_and_disable_is_idempotent_for_cli() {
        let (_dir, service, user_id) = test_service().await;
        let enrollment = service
            .begin_enrollment("session-1", &user_id, "admin")
            .await
            .unwrap();
        let secret = Secret::Encoded(enrollment.secret.clone());
        let code = totp(&secret, "admin").unwrap().generate_current().unwrap();
        service
            .confirm_enrollment("session-1", &user_id, &enrollment.id, &code)
            .await
            .unwrap();

        let restarted = TwoFactorService::new(service.pool.clone());
        assert!(restarted.is_enabled(&user_id).await.unwrap());
        restarted.disable(&user_id, &code).await.unwrap();
        assert!(!restarted.is_enabled(&user_id).await.unwrap());
        assert!(!restarted.disable_without_code(&user_id).await.unwrap());
    }
}
