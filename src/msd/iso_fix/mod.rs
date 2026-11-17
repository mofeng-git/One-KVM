//! Re-master "UDF-only" or Joliet-less ISO images into standard
//! ISO 9660 + Joliet images that Windows can read through the virtual CD-ROM
//! drive, preserving the El Torito BIOS and UEFI boot entries.
//!
//! This is a Rust port of `docs/iso-fix/onekvm-iso-fix.sh`. The content of the
//! source image is mounted read-only (UDF or ISO 9660), then `xorriso` rebuilds
//! a Joliet image from the mounted tree. `xorriso` is auto-installed through
//! `apt-get` when it is missing, matching the shell script.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::iso::{IsoFormat, ISO_SHELL_MIN_FILES};
use super::types::{ConversionProgress, ConversionStage, ConversionStatus, ImageInfo};
use crate::diagnostics::get_disk_space;
use crate::error::{AppError, MsdErrorCode, Result};
use crate::events::{EventBus, SystemEvent};

mod boot;
mod process;
mod verify;

use boot::{find_boot_options, has_boot_entry};
use process::{
    cancelled_error, catalog_boot_options, find_in_path, mount_image, run_simple, run_xorriso,
    MountGuard, MountType,
};
use verify::{count_mounted_files, probe_image, verify_output};

const CONVERSION_SUFFIX: &str = "_joliet";
const SPACE_REQUIRED_NUMERATOR: u64 = 11;
const SPACE_REQUIRED_DENOMINATOR: u64 = 10;
const APT_TIMEOUT: Duration = Duration::from_secs(600);
const PROGRESS_POLL_INTERVAL: Duration = Duration::from_millis(500);

struct ConversionJob {
    progress: ConversionProgress,
    cancel: CancellationToken,
}

impl ConversionJob {
    fn is_running(&self) -> bool {
        self.progress.status.is_running()
    }
}

/// Manages asynchronous ISO conversion jobs.
///
/// Conversions are serialized: only one job runs at a time to avoid competing
/// for disk throughput and loop devices on embedded hardware.
#[derive(Clone)]
pub struct IsoConversionManager {
    images_path: Arc<PathBuf>,
    jobs: Arc<RwLock<HashMap<String, ConversionJob>>>,
    events: Arc<RwLock<Option<Arc<EventBus>>>>,
}

impl IsoConversionManager {
    pub fn new(images_path: PathBuf) -> Self {
        Self {
            images_path: Arc::new(images_path),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
    }

    pub async fn is_running(&self, image_id: &str) -> bool {
        self.jobs
            .read()
            .await
            .get(image_id)
            .is_some_and(ConversionJob::is_running)
    }

    pub async fn get(&self, image_id: &str) -> Option<ConversionProgress> {
        self.jobs
            .read()
            .await
            .get(image_id)
            .map(|job| job.progress.clone())
    }

    pub async fn list(&self) -> Vec<ConversionProgress> {
        let mut values: Vec<_> = self
            .jobs
            .read()
            .await
            .values()
            .map(|job| job.progress.clone())
            .collect();
        values.sort_by(|left, right| left.source_name.cmp(&right.source_name));
        values
    }

    pub async fn cancel(&self, image_id: &str) -> Result<()> {
        let jobs = self.jobs.read().await;
        let job = jobs
            .get(image_id)
            .ok_or_else(|| AppError::from(MsdErrorCode::MsdResourceNotFound))?;
        if !job.is_running() {
            return Err(MsdErrorCode::MsdResourceNotFound.into());
        }
        job.cancel.cancel();
        Ok(())
    }

    pub async fn cancel_all(&self) {
        for job in self.jobs.read().await.values() {
            if job.is_running() {
                job.cancel.cancel();
            }
        }
    }

    pub async fn start(&self, image: &ImageInfo) -> Result<ConversionProgress> {
        if !is_iso_name(&image.name) {
            return Err(MsdErrorCode::MsdInvalidRequest.into());
        }
        if self.any_running().await {
            return Err(MsdErrorCode::MsdOperationInProgress.into());
        }

        let probe = probe_image(&image.path).await?;
        if !probe.compatibility.needs_conversion {
            return Err(MsdErrorCode::MsdIsoConversionNotNeeded.into());
        }
        if !probe.compatibility.convertible {
            return Err(MsdErrorCode::MsdIsoConversionUnsupported.into());
        }

        let output_name = conversion_output_name(&image.name);
        let output_path = self.images_path.join(&output_name);
        if output_path.exists() {
            return Err(MsdErrorCode::MsdResourceAlreadyExists.into());
        }
        ensure_conversion_space(&self.images_path, image.size)?;

        let progress = ConversionProgress {
            conversion_id: uuid::Uuid::new_v4().to_string(),
            image_id: image.id.clone(),
            source_name: image.name.clone(),
            output_name: Some(output_name.clone()),
            stage: ConversionStage::Preparing,
            status: ConversionStatus::Started,
            progress_pct: Some(0.0),
            error_code: None,
        };
        let cancel = CancellationToken::new();

        {
            let mut jobs = self.jobs.write().await;
            if jobs.values().any(ConversionJob::is_running) {
                return Err(MsdErrorCode::MsdOperationInProgress.into());
            }
            jobs.retain(|_, job| job.is_running());
            jobs.insert(
                image.id.clone(),
                ConversionJob {
                    progress: progress.clone(),
                    cancel: cancel.clone(),
                },
            );
        }

        self.publish(&progress).await;

        let manager = self.clone();
        let image = image.clone();
        tokio::spawn(async move {
            let result = manager.run_conversion(&image, &output_name, &cancel).await;
            manager.finish(&image, result, &cancel).await;
        });

        Ok(progress)
    }

    async fn any_running(&self) -> bool {
        self.jobs
            .read()
            .await
            .values()
            .any(ConversionJob::is_running)
    }

    async fn finish(&self, image: &ImageInfo, result: Result<()>, cancel: &CancellationToken) {
        if let Err(error) = result {
            if cancel.is_cancelled() {
                self.report(
                    &image.id,
                    ConversionStage::Cancelled,
                    ConversionStatus::Cancelled,
                    None,
                    None,
                )
                .await;
                info!(image = %image.name, "ISO conversion cancelled");
            } else {
                let code = match &error {
                    AppError::Msd(error) => error.code(),
                    _ => MsdErrorCode::MsdIsoConversionFailed,
                };
                warn!(image = %image.name, %error, "ISO conversion failed");
                self.report(
                    &image.id,
                    ConversionStage::Failed,
                    ConversionStatus::Failed,
                    None,
                    Some(code),
                )
                .await;
            }
            return;
        }

        self.report(
            &image.id,
            ConversionStage::Completed,
            ConversionStatus::Completed,
            Some(100.0),
            None,
        )
        .await;
        info!(image = %image.name, "ISO conversion completed");
    }

    async fn run_conversion(
        &self,
        image: &ImageInfo,
        output_name: &str,
        cancel: &CancellationToken,
    ) -> Result<()> {
        self.report(
            &image.id,
            ConversionStage::Preparing,
            ConversionStatus::InProgress,
            Some(1.0),
            None,
        )
        .await;

        let probe = probe_image(&image.path).await?;
        let mount_type = match probe.compatibility.format {
            IsoFormat::UdfOnly => MountType::Udf,
            IsoFormat::MissingJoliet => MountType::Iso9660,
            _ => return Err(MsdErrorCode::MsdIsoConversionUnsupported.into()),
        };

        self.ensure_tools(&image.id, cancel).await?;
        if cancel.is_cancelled() {
            return Err(cancelled_error());
        }

        let mount_dir =
            std::env::temp_dir().join(format!("one-kvm-isofix-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&mount_dir)
            .await
            .map_err(|error| {
                warn!(%error, "Failed to create ISO conversion mount directory");
                AppError::from(MsdErrorCode::MsdIsoConversionFailed)
            })?;
        let mut mount_guard = MountGuard::new(mount_dir.clone());

        self.report(
            &image.id,
            ConversionStage::Mounting,
            ConversionStatus::InProgress,
            Some(10.0),
            None,
        )
        .await;
        mount_image(&image.path, &mount_dir, mount_type, cancel).await?;
        mount_guard.mark_mounted();

        let mut boot_args = find_boot_options(&mount_dir);
        if !has_boot_entry(&boot_args) {
            boot_args = catalog_boot_options(&image.path, cancel).await;
        }
        if cancel.is_cancelled() {
            mount_guard.unmount().await;
            return Err(cancelled_error());
        }

        let count_dir = mount_dir.clone();
        let (mounted_files, mounted_files_truncated) =
            tokio::task::spawn_blocking(move || count_mounted_files(&count_dir))
                .await
                .map_err(|error| {
                    warn!(%error, "Failed to count mounted ISO files");
                    AppError::from(MsdErrorCode::MsdIsoConversionFailed)
                })?;
        if mounted_files == 0 {
            mount_guard.unmount().await;
            warn!(image = %image.name, "Mounted ISO contains no files");
            return Err(MsdErrorCode::MsdIsoConversionUnsupported.into());
        }
        let required_files = if mounted_files_truncated {
            ISO_SHELL_MIN_FILES
        } else {
            mounted_files
        };

        let fallback_label = Path::new(&image.name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("ONE-KVM");
        let label = sanitize_volume_label(probe.volume_label.as_deref(), fallback_label);
        let part_path = self
            .images_path
            .join(format!(".convert-{}.part", uuid::Uuid::new_v4()));

        self.report(
            &image.id,
            ConversionStage::Remastering,
            ConversionStatus::InProgress,
            Some(15.0),
            None,
        )
        .await;
        let remaster_result = self
            .remaster(image, &mount_dir, &part_path, &label, &boot_args, cancel)
            .await;
        mount_guard.unmount().await;

        if let Err(error) = remaster_result {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(error);
        }
        if cancel.is_cancelled() {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(cancelled_error());
        }

        self.report(
            &image.id,
            ConversionStage::Verifying,
            ConversionStatus::InProgress,
            Some(90.0),
            None,
        )
        .await;
        if let Err(error) = verify_output(&part_path, &image.path, required_files, cancel).await {
            // Keep the partial output for diagnostics, matching the shell script.
            warn!(
                image = %image.name,
                part = %part_path.display(),
                %error,
                "Converted ISO failed verification"
            );
            return Err(error);
        }
        if cancel.is_cancelled() {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(cancelled_error());
        }

        let final_path = self.images_path.join(output_name);
        if tokio::fs::try_exists(&final_path).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(MsdErrorCode::MsdResourceAlreadyExists.into());
        }
        tokio::fs::rename(&part_path, &final_path)
            .await
            .map_err(|error| {
                warn!(%error, "Failed to commit converted ISO");
                let _ = std::fs::remove_file(&part_path);
                AppError::from(MsdErrorCode::MsdIsoConversionFailed)
            })?;

        Ok(())
    }

    async fn remaster(
        &self,
        image: &ImageInfo,
        mount_dir: &Path,
        part_path: &Path,
        label: &str,
        boot_args: &[String],
        cancel: &CancellationToken,
    ) -> Result<()> {
        let monitor_token = CancellationToken::new();
        let monitor = self.spawn_size_monitor(
            image,
            part_path.to_path_buf(),
            monitor_token.clone(),
            cancel.clone(),
        );

        let mut command = Command::new("xorriso");
        command.args([
            "-as",
            "mkisofs",
            "-iso-level",
            "3",
            "-J",
            "-joliet-long",
            "-r",
            "-V",
            label,
        ]);
        command.args(boot_args);
        command.args(["-hide", "boot.catalog", "-hide-joliet", "boot.catalog"]);
        command.arg("-o").arg(part_path).arg(mount_dir);

        let result = run_xorriso(command, cancel).await;
        monitor_token.cancel();
        let _ = monitor.await;

        match result {
            Ok(process) if process.status.success() => Ok(()),
            Ok(process) => {
                let log = process.log_tail.into_iter().collect::<Vec<_>>().join(" | ");
                warn!(status = %process.status, log = %log, "xorriso failed to rebuild the ISO");
                Err(MsdErrorCode::MsdIsoConversionFailed.into())
            }
            Err(error) => Err(error),
        }
    }

    fn spawn_size_monitor(
        &self,
        image: &ImageInfo,
        part_path: PathBuf,
        stop: CancellationToken,
        parent: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        let manager = self.clone();
        let image_id = image.id.clone();
        let total = image.size.max(1);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(PROGRESS_POLL_INTERVAL);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    _ = parent.cancelled() => break,
                    _ = ticker.tick() => {
                        let Ok(metadata) = tokio::fs::metadata(&part_path).await else {
                            continue;
                        };
                        let ratio = (metadata.len() as f32 / total as f32).clamp(0.0, 1.0);
                        manager
                            .report(
                                &image_id,
                                ConversionStage::Remastering,
                                ConversionStatus::InProgress,
                                Some(15.0 + ratio * 70.0),
                                None,
                            )
                            .await;
                    }
                }
            }
        })
    }

    async fn ensure_tools(&self, image_id: &str, cancel: &CancellationToken) -> Result<()> {
        if find_in_path("xorriso").is_some() {
            return Ok(());
        }

        self.report(
            image_id,
            ConversionStage::InstallingTools,
            ConversionStatus::InProgress,
            Some(5.0),
            None,
        )
        .await;

        if find_in_path("apt-get").is_none() {
            warn!("xorriso is missing and apt-get is unavailable");
            return Err(MsdErrorCode::MsdIsoToolsMissing.into());
        }

        let mut update = Command::new("apt-get");
        update
            .args(["update", "-qq"])
            .env("DEBIAN_FRONTEND", "noninteractive");
        let _ = run_simple(update, cancel, APT_TIMEOUT).await;

        if cancel.is_cancelled() {
            return Err(cancelled_error());
        }

        let mut install = Command::new("apt-get");
        install
            .args(["install", "-y", "-qq", "xorriso"])
            .env("DEBIAN_FRONTEND", "noninteractive");
        let output = run_simple(install, cancel, APT_TIMEOUT).await?;
        if !output.status.success() || find_in_path("xorriso").is_none() {
            warn!(
                stderr = %String::from_utf8_lossy(&output.stderr).trim(),
                "Failed to install xorriso"
            );
            return Err(MsdErrorCode::MsdIsoToolsMissing.into());
        }

        info!("Installed xorriso for ISO conversion");
        Ok(())
    }

    async fn report(
        &self,
        image_id: &str,
        stage: ConversionStage,
        status: ConversionStatus,
        progress_pct: Option<f32>,
        error_code: Option<MsdErrorCode>,
    ) {
        let progress = {
            let mut jobs = self.jobs.write().await;
            let Some(job) = jobs.get_mut(image_id) else {
                return;
            };

            let rounded = progress_pct.map(|value| (value.clamp(0.0, 100.0) * 10.0).round() / 10.0);
            let error = error_code.map(|code| code.as_str().to_string());
            let stage_changed = job.progress.stage != stage;
            let status_changed = job.progress.status != status;
            let error_changed = job.progress.error_code != error;
            let percent_changed = match (job.progress.progress_pct, rounded) {
                (Some(current), Some(next)) => (current - next).abs() >= 1.0,
                (None, Some(_)) | (Some(_), None) => true,
                (None, None) => false,
            };
            if !stage_changed && !status_changed && !error_changed && !percent_changed {
                return;
            }

            job.progress.stage = stage;
            job.progress.status = status;
            job.progress.progress_pct = rounded;
            job.progress.error_code = error;
            job.progress.clone()
        };

        self.publish(&progress).await;
    }

    async fn publish(&self, progress: &ConversionProgress) {
        let events = self.events.read().await.clone();
        let Some(bus) = events else {
            return;
        };
        bus.publish(SystemEvent::MsdConversionProgress {
            conversion_id: progress.conversion_id.clone(),
            image_id: progress.image_id.clone(),
            source_name: progress.source_name.clone(),
            output_name: progress.output_name.clone(),
            stage: progress.stage.as_str().to_string(),
            status: progress.status.as_str().to_string(),
            progress_pct: progress.progress_pct,
            error_code: progress.error_code.clone(),
        });
    }
}

fn ensure_conversion_space(images_path: &Path, source_size: u64) -> Result<()> {
    let required =
        source_size.saturating_mul(SPACE_REQUIRED_NUMERATOR) / SPACE_REQUIRED_DENOMINATOR;
    let space = get_disk_space(images_path).map_err(|error| {
        warn!(%error, "Failed to read MSD storage space for ISO conversion");
        AppError::from(MsdErrorCode::MsdStorageSpaceUnavailable)
    })?;
    if space.available < required {
        warn!(
            available = space.available,
            required, "Not enough free space for ISO conversion"
        );
        return Err(MsdErrorCode::MsdStorageFull.into());
    }
    Ok(())
}

fn is_iso_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".iso")
}

fn conversion_output_name(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("image")
        .trim_start_matches('.')
        .trim();
    let stem = if stem.is_empty() { "image" } else { stem };
    format!("{stem}{CONVERSION_SUFFIX}.iso")
}

fn sanitize_volume_label(volume_label: Option<&str>, fallback: &str) -> String {
    let source = volume_label.unwrap_or("").trim();
    let source = if source.is_empty() { fallback } else { source };
    let label: String = source
        .chars()
        .map(|character| {
            if (character.is_ascii_graphic() || character == ' ')
                && !matches!(character, '/' | '\\' | '"' | '\'')
            {
                character
            } else {
                '_'
            }
        })
        .take(32)
        .collect();
    let label = label.trim();
    if label.is_empty() {
        "ONE-KVM".to_string()
    } else {
        label.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_output_name_from_source_name() {
        assert_eq!(conversion_output_name("win10.iso"), "win10_joliet.iso");
        assert_eq!(conversion_output_name("WIN10.ISO"), "WIN10_joliet.iso");
        assert_eq!(conversion_output_name(".hidden.iso"), "hidden_joliet.iso");
        assert_eq!(conversion_output_name("image"), "image_joliet.iso");
    }

    #[test]
    fn recognizes_iso_extension_case_insensitively() {
        assert!(is_iso_name("image.iso"));
        assert!(is_iso_name("image.ISO"));
        assert!(!is_iso_name("image.img"));
    }

    #[test]
    fn sanitizes_and_truncates_volume_labels() {
        assert_eq!(sanitize_volume_label(Some("WIN_10"), "fallback"), "WIN_10");
        assert_eq!(sanitize_volume_label(None, "fallback"), "fallback");
        assert_eq!(sanitize_volume_label(Some("  "), "fallback"), "fallback");
        assert_eq!(
            sanitize_volume_label(Some("A_LABEL_THAT_IS_MUCH_TOO_LONG_FOR_JOLIET"), "x"),
            "A_LABEL_THAT_IS_MUCH_TOO_LONG_FO"
        );
        assert_eq!(
            sanitize_volume_label(Some("bad/name"), "fallback"),
            "bad_name"
        );
    }

    #[test]
    fn conversion_progress_serializes_stage_names() {
        let progress = ConversionProgress {
            conversion_id: "id".into(),
            image_id: "image".into(),
            source_name: "source.iso".into(),
            output_name: Some("source_joliet.iso".into()),
            stage: ConversionStage::Remastering,
            status: ConversionStatus::InProgress,
            progress_pct: Some(42.5),
            error_code: None,
        };
        let json = serde_json::to_value(progress).unwrap();

        assert_eq!(json["stage"], "remastering");
        assert_eq!(json["status"], "in_progress");
        assert_eq!(json["progress_pct"], 42.5);
        assert!(json.get("error_code").is_none());
    }
}
