use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, RwLock, Semaphore};

use crate::error::{AppError, Result};

const DEFAULT_UPDATE_BASE_URL: &str = "https://update.one-kvm.cn";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl Default for UpdateChannel {
    fn default() -> Self {
        Self::Stable
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelsManifest {
    pub stable: String,
    pub beta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleasesManifest {
    pub releases: Vec<ReleaseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub channel: UpdateChannel,
    pub published_at: String,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub artifacts: HashMap<String, ArtifactInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseNotesItem {
    pub version: String,
    pub published_at: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOverviewResponse {
    pub success: bool,
    pub current_version: String,
    pub channel: UpdateChannel,
    pub latest_version: String,
    pub upgrade_available: bool,
    pub target_version: Option<String>,
    pub notes_between: Vec<ReleaseNotesItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeRequest {
    pub channel: Option<UpdateChannel>,
    pub target_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UpdatePhase {
    Idle,
    Checking,
    Downloading,
    Verifying,
    Installing,
    Restarting,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatusResponse {
    pub success: bool,
    pub phase: UpdatePhase,
    pub progress: u8,
    pub current_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

pub struct UpdateService {
    client: reqwest::Client,
    base_url: String,
    work_dir: PathBuf,
    status: RwLock<UpdateStatusResponse>,
    upgrade_permit: Arc<Semaphore>,
}

impl UpdateService {
    pub fn new(work_dir: PathBuf) -> Self {
        let base_url = std::env::var("ONE_KVM_UPDATE_BASE_URL")
            .ok()
            .filter(|url| !url.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_UPDATE_BASE_URL.to_string());

        Self {
            client: reqwest::Client::new(),
            base_url,
            work_dir,
            status: RwLock::new(UpdateStatusResponse {
                success: true,
                phase: UpdatePhase::Idle,
                progress: 0,
                current_version: env!("CARGO_PKG_VERSION").to_string(),
                target_version: None,
                message: None,
                last_error: None,
            }),
            upgrade_permit: Arc::new(Semaphore::new(1)),
        }
    }

    pub async fn status(&self) -> UpdateStatusResponse {
        self.status.read().await.clone()
    }

    pub async fn overview(&self, channel: UpdateChannel) -> Result<UpdateOverviewResponse> {
        let channels: ChannelsManifest = self.fetch_json("/v1/channels.json").await?;
        let releases: ReleasesManifest = self.fetch_json("/v1/releases.json").await?;

        let current_version = parse_version(env!("CARGO_PKG_VERSION"))?;
        let latest_version_str = match channel {
            UpdateChannel::Stable => channels.stable,
            UpdateChannel::Beta => channels.beta,
        };
        let latest_version = parse_version(&latest_version_str)?;
        let current_parts = parse_version_parts(&current_version)?;
        let latest_parts = parse_version_parts(&latest_version)?;

        let mut notes_between = Vec::new();
        for release in &releases.releases {
            if release.channel != channel {
                continue;
            }
            let version = match parse_version(&release.version) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let version_parts = match parse_version_parts(&version) {
                Ok(parts) => parts,
                Err(_) => continue,
            };
            if compare_version_parts(&version_parts, &current_parts) == std::cmp::Ordering::Greater
                && compare_version_parts(&version_parts, &latest_parts)
                    != std::cmp::Ordering::Greater
            {
                notes_between.push((
                    version_parts,
                    ReleaseNotesItem {
                        version: release.version.clone(),
                        published_at: release.published_at.clone(),
                        notes: release.notes.clone(),
                    },
                ));
            }
        }

        notes_between.sort_by(|a, b| compare_version_parts(&a.0, &b.0));
        let notes_between = notes_between.into_iter().map(|(_, item)| item).collect();

        let upgrade_available =
            compare_versions(&latest_version, &current_version)? == std::cmp::Ordering::Greater;

        Ok(UpdateOverviewResponse {
            success: true,
            current_version: current_version.to_string(),
            channel,
            latest_version: latest_version.clone(),
            upgrade_available,
            target_version: if upgrade_available {
                Some(latest_version)
            } else {
                None
            },
            notes_between,
        })
    }

    pub fn start_upgrade(
        self: &Arc<Self>,
        req: UpgradeRequest,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Result<()> {
        if req.channel.is_none() == req.target_version.is_none() {
            return Err(AppError::BadRequest(
                "Provide exactly one of channel or target_version".to_string(),
            ));
        }

        let permit = self
            .upgrade_permit
            .clone()
            .try_acquire_owned()
            .map_err(|_| AppError::BadRequest("Upgrade is already running".to_string()))?;

        let service = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = service.execute_upgrade(req, shutdown_tx).await {
                service
                    .set_status(
                        UpdatePhase::Failed,
                        0,
                        None,
                        Some(e.to_string()),
                        Some(e.to_string()),
                    )
                    .await;
            }
        });

        Ok(())
    }

    async fn execute_upgrade(
        &self,
        req: UpgradeRequest,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Result<()> {
        self.set_status(
            UpdatePhase::Checking,
            5,
            None,
            Some("Checking for updates".to_string()),
            None,
        )
        .await;

        let channels: ChannelsManifest = self.fetch_json("/v1/channels.json").await?;
        let releases: ReleasesManifest = self.fetch_json("/v1/releases.json").await?;

        let current_version = parse_version(env!("CARGO_PKG_VERSION"))?;
        let target_version = if let Some(channel) = req.channel {
            let version_str = match channel {
                UpdateChannel::Stable => channels.stable,
                UpdateChannel::Beta => channels.beta,
            };
            parse_version(&version_str)?
        } else {
            parse_version(req.target_version.as_deref().unwrap_or_default())?
        };

        if compare_versions(&target_version, &current_version)? != std::cmp::Ordering::Greater {
            return Err(AppError::BadRequest(format!(
                "Target version {} must be greater than current version {}",
                target_version, current_version
            )));
        }

        let target_release = releases
            .releases
            .iter()
            .find(|r| r.version == target_version)
            .ok_or_else(|| AppError::NotFound(format!("Release {} not found", target_version)))?;

        let target_triple = current_target_triple()?;
        let artifact = target_release
            .artifacts
            .get(&target_triple)
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "No binary for target {} in version {}",
                    target_triple, target_version
                ))
            })?
            .clone();

        self.set_status(
            UpdatePhase::Downloading,
            10,
            Some(target_version.clone()),
            Some("Downloading binary".to_string()),
            None,
        )
        .await;

        tokio::fs::create_dir_all(&self.work_dir).await?;
        let staging_path = self
            .work_dir
            .join(format!("one-kvm-{}-download", target_version));

        let artifact_url = self.resolve_url(&artifact.url);
        self.download_and_verify(&artifact_url, &staging_path, &artifact)
            .await?;

        self.set_status(
            UpdatePhase::Installing,
            80,
            Some(target_version.clone()),
            Some("Replacing binary".to_string()),
            None,
        )
        .await;

        self.install_binary(&staging_path).await?;

        self.set_status(
            UpdatePhase::Restarting,
            95,
            Some(target_version),
            Some("Restarting service".to_string()),
            None,
        )
        .await;

        let _ = shutdown_tx.send(());
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        restart_current_process()?;
        Ok(())
    }

    async fn download_and_verify(
        &self,
        url: &str,
        output_path: &Path,
        artifact: &ArtifactInfo,
    ) -> Result<()> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to download {}: {}", url, e)))?
            .error_for_status()
            .map_err(|e| AppError::Internal(format!("Download request failed: {}", e)))?;

        let mut file = tokio::fs::File::create(output_path).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| AppError::Internal(format!("Read download stream failed: {}", e)))?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if artifact.size > 0 {
                let ratio = (downloaded as f64 / artifact.size as f64).clamp(0.0, 1.0);
                let progress = 10 + (ratio * 60.0) as u8;
                self.set_status(
                    UpdatePhase::Downloading,
                    progress,
                    None,
                    Some(format!(
                        "Downloading binary ({} / {} bytes)",
                        downloaded, artifact.size
                    )),
                    None,
                )
                .await;
            }
        }
        file.flush().await?;

        if artifact.size > 0 && downloaded != artifact.size {
            return Err(AppError::Internal(format!(
                "Downloaded size mismatch: expected {}, got {}",
                artifact.size, downloaded
            )));
        }

        self.set_status(
            UpdatePhase::Verifying,
            72,
            None,
            Some("Verifying sha256".to_string()),
            None,
        )
        .await;

        let actual_sha256 = compute_file_sha256(output_path).await?;
        let expected_sha256 = normalize_sha256(&artifact.sha256).ok_or_else(|| {
            AppError::Internal(format!(
                "Invalid sha256 format in manifest: {}",
                artifact.sha256
            ))
        })?;
        if actual_sha256 != expected_sha256 {
            return Err(AppError::Internal(format!(
                "SHA256 mismatch: expected {}, got {}",
                expected_sha256, actual_sha256
            )));
        }

        Ok(())
    }

    async fn install_binary(&self, staging_path: &Path) -> Result<()> {
        let current_exe = std::env::current_exe()
            .map_err(|e| AppError::Internal(format!("Failed to get current exe path: {}", e)))?;
        let exe_dir = current_exe.parent().ok_or_else(|| {
            AppError::Internal("Failed to determine executable directory".to_string())
        })?;

        let install_path = exe_dir.join("one-kvm.upgrade.new");

        tokio::fs::copy(staging_path, &install_path)
            .await
            .map_err(|e| {
                AppError::Internal(format!("Failed to stage binary into install path: {}", e))
            })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&install_path).await?.permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&install_path, perms).await?;
        }

        tokio::fs::rename(&install_path, &current_exe)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to replace executable {}", e)))?;

        Ok(())
    }

    async fn fetch_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to fetch {}: {}", url, e)))?
            .error_for_status()
            .map_err(|e| AppError::Internal(format!("Request failed {}: {}", url, e)))?;

        response
            .json::<T>()
            .await
            .map_err(|e| AppError::Internal(format!("Invalid update response {}: {}", url, e)))
    }

    fn resolve_url(&self, url: &str) -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!(
                "{}/{}",
                self.base_url.trim_end_matches('/'),
                url.trim_start_matches('/')
            )
        }
    }

    async fn set_status(
        &self,
        phase: UpdatePhase,
        progress: u8,
        target_version: Option<String>,
        message: Option<String>,
        last_error: Option<String>,
    ) {
        let mut status = self.status.write().await;
        status.phase = phase;
        status.progress = progress;
        if target_version.is_some() {
            status.target_version = target_version;
        }
        status.message = message;
        status.last_error = last_error;
        status.success = status.phase != UpdatePhase::Failed;
        status.current_version = env!("CARGO_PKG_VERSION").to_string();
    }
}

fn parse_version(input: &str) -> Result<String> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() != 3 {
        return Err(AppError::Internal(format!(
            "Invalid version {}, expected x.x.x",
            input
        )));
    }
    if parts
        .iter()
        .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(AppError::Internal(format!(
            "Invalid version {}, expected numeric x.x.x",
            input
        )));
    }
    Ok(input.to_string())
}

fn compare_versions(a: &str, b: &str) -> Result<std::cmp::Ordering> {
    let pa = parse_version_parts(a)?;
    let pb = parse_version_parts(b)?;
    Ok(compare_version_parts(&pa, &pb))
}

fn parse_version_parts(input: &str) -> Result<[u64; 3]> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() != 3 {
        return Err(AppError::Internal(format!(
            "Invalid version {}, expected x.x.x",
            input
        )));
    }
    let major = parts[0]
        .parse::<u64>()
        .map_err(|e| AppError::Internal(format!("Invalid major version {}: {}", parts[0], e)))?;
    let minor = parts[1]
        .parse::<u64>()
        .map_err(|e| AppError::Internal(format!("Invalid minor version {}: {}", parts[1], e)))?;
    let patch = parts[2]
        .parse::<u64>()
        .map_err(|e| AppError::Internal(format!("Invalid patch version {}: {}", parts[2], e)))?;
    Ok([major, minor, patch])
}

fn compare_version_parts(a: &[u64; 3], b: &[u64; 3]) -> std::cmp::Ordering {
    a[0].cmp(&b[0]).then(a[1].cmp(&b[1])).then(a[2].cmp(&b[2]))
}

async fn compute_file_sha256(path: &Path) -> Result<String> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn normalize_sha256(input: &str) -> Option<String> {
    let token = input.split_whitespace().next()?.trim().to_lowercase();
    if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(token)
}

fn current_target_triple() -> Result<String> {
    let triple = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "arm") => "armv7-unknown-linux-gnueabihf",
        _ => {
            return Err(AppError::BadRequest(format!(
                "Unsupported platform {}-{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )));
        }
    };
    Ok(triple.to_string())
}

fn restart_current_process() -> Result<()> {
    let exe = std::env::current_exe()
        .map_err(|e| AppError::Internal(format!("Failed to get current exe: {}", e)))?;
    let args: Vec<String> = std::env::args().skip(1).collect();

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&exe).args(&args).exec();
        Err(AppError::Internal(format!("Failed to restart: {}", err)))
    }

    #[cfg(not(unix))]
    {
        std::process::Command::new(&exe)
            .args(&args)
            .spawn()
            .map_err(|e| AppError::Internal(format!("Failed to spawn restart process: {}", e)))?;
        std::process::exit(0);
    }
}
