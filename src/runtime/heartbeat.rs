//! Silent device heartbeat.
//!
//! Sends a small identity payload to the project server every 15 minutes so the
//! online status of a device can be inspected from the report center. Failures
//! are intentionally ignored and only recorded at debug level: the heartbeat
//! must never disturb the user or the web UI.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use crate::platform::PlatformMode;
use crate::utils::hostname_uname;

/// Heartbeat receiver. Kept separate from the report endpoint so both can move
/// independently.
pub(super) const HEARTBEAT_ENDPOINT: &str =
    "http://api.mrcong.cn:300/apps/one-kvm/heartbeat.php?app=one-kvm";

/// Default heartbeat period: 15 minutes.
pub(super) const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Wait a little after startup so boot-time work is not delayed.
const INITIAL_DELAY: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Serialize)]
struct HeartbeatPayload<'a> {
    id: &'a str,
    name: &'a str,
    version: &'a str,
    platform: &'a str,
}

/// Spawn the background heartbeat task. The task is aborted by the runtime
/// supervisor during shutdown.
pub(super) fn spawn(data_dir: &Path) -> tokio::task::JoinHandle<()> {
    let data_dir = data_dir.to_path_buf();

    let task = tokio::spawn(async move {
        let device_id = load_or_create_device_id(&data_dir).await;
        let client = match reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("one-kvm/", env!("CARGO_PKG_VERSION")))
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                tracing::debug!("Heartbeat client unavailable: {}", error);
                return;
            }
        };

        let name = hostname_uname();
        let version = env!("CARGO_PKG_VERSION");
        let platform = PlatformMode::current().label();
        let payload = HeartbeatPayload {
            id: &device_id,
            name: &name,
            version,
            platform,
        };

        tokio::time::sleep(INITIAL_DELAY).await;
        loop {
            send_once(&client, &payload).await;
            tokio::time::sleep(HEARTBEAT_INTERVAL).await;
        }
    });

    tracing::debug!("Heartbeat task started");
    task
}

async fn send_once(client: &reqwest::Client, payload: &HeartbeatPayload<'_>) {
    match client.post(HEARTBEAT_ENDPOINT).json(payload).send().await {
        Ok(response) if response.status().is_success() => {
            tracing::debug!(status = response.status().as_u16(), "Heartbeat sent");
        }
        Ok(response) => {
            tracing::debug!(status = response.status().as_u16(), "Heartbeat was not accepted");
        }
        Err(error) => {
            tracing::debug!("Heartbeat failed: {}", error);
        }
    }
}

/// Read the persisted device id, or create one on first run.
///
/// The id is a random UUID stored under the data directory, so it survives
/// restarts but carries no hardware or user information.
async fn load_or_create_device_id(data_dir: &Path) -> String {
    let path = data_dir.join("device-id");

    if let Ok(existing) = tokio::fs::read_to_string(&path).await {
        let existing = existing.trim();
        if is_valid_device_id(existing) {
            return existing.to_string();
        }
    }

    let id = uuid::Uuid::new_v4().simple().to_string();
    if let Err(error) = persist_device_id(data_dir, &path, &id).await {
        tracing::debug!("Could not persist device id: {}", error);
    }

    id
}

async fn persist_device_id(data_dir: &Path, path: &Path, id: &str) -> std::io::Result<()> {
    tokio::fs::create_dir_all(data_dir).await?;

    // Write to a temporary file first so a crash cannot leave a partial id.
    let temp = data_dir.join(".device-id.tmp");
    tokio::fs::write(&temp, format!("{id}\n")).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        let _ = tokio::fs::set_permissions(&temp, permissions).await;
    }

    // Windows cannot rename over an existing file. The existing file is only
    // removed when it was missing or invalid, so this is safe on every platform.
    let _ = tokio::fs::remove_file(path).await;
    if let Err(error) = tokio::fs::rename(&temp, path).await {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(error);
    }

    Ok(())
}

fn is_valid_device_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_interval_is_fifteen_minutes() {
        assert_eq!(HEARTBEAT_INTERVAL, Duration::from_secs(900));
    }

    #[test]
    fn endpoint_uses_app_scoped_port_300_url() {
        assert!(HEARTBEAT_ENDPOINT.contains("api.mrcong.cn:300"));
        assert!(HEARTBEAT_ENDPOINT.contains("/apps/one-kvm/heartbeat.php"));
    }

    #[test]
    fn device_id_validation_accepts_generated_id() {
        let id = uuid::Uuid::new_v4().simple().to_string();
        assert!(is_valid_device_id(&id));
        assert!(!is_valid_device_id("short"));
        assert!(!is_valid_device_id(&"z".repeat(32)));
    }

    #[test]
    fn payload_serializes_identity_fields() {
        let payload = HeartbeatPayload {
            id: "0123456789abcdef0123456789abcdef",
            name: "one-kvm",
            version: "0.2.9",
            platform: "Linux",
        };
        let value = serde_json::to_value(payload).unwrap();

        assert_eq!(value["id"], "0123456789abcdef0123456789abcdef");
        assert_eq!(value["name"], "one-kvm");
        assert_eq!(value["version"], "0.2.9");
        assert_eq!(value["platform"], "Linux");
    }

    #[tokio::test]
    async fn device_id_is_stable_between_calls() {
        let dir = tempfile::tempdir().unwrap();
        let first = load_or_create_device_id(dir.path()).await;
        let second = load_or_create_device_id(dir.path()).await;

        assert_eq!(first, second);
        assert!(is_valid_device_id(&first));
        assert!(dir.path().join("device-id").is_file());
    }
}
