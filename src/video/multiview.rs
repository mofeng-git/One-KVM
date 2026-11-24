//! Source registry coordinated exclusively by VideoStreamManager.
use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::debug;
use typeshare::typeshare;

use crate::config::{StreamMode, VideoConfig, VideoSourceConfig};
use crate::error::{AppError, Result};
use crate::events::SystemEvent;
use crate::video::device::{self, VideoControlMode, VideoDeviceInfo};
use crate::video::format::{PixelFormat, Resolution};
use crate::video::streamer::NO_AUTO_DEVICE_MESSAGE;
use crate::video::{Streamer, VideoStreamManager};

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub id: String,
    pub label: String,
    pub device: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u32,
    pub enabled: bool,
    pub primary: bool,
}

#[derive(Clone)]
pub(crate) struct Source {
    pub info: SourceInfo,
    pub config: VideoSourceConfig,
    pub streamer: Arc<Streamer>,
    pub apply_error: Option<String>,
    // Snapshot ownership changes on every registry replacement, including ABA edits.
    pub revision: Arc<()>,
}

impl Source {
    pub(crate) fn new(config: VideoSourceConfig, primary: bool, streamer: Arc<Streamer>) -> Self {
        Self {
            info: SourceInfo {
                id: config.id.clone(),
                label: if config.label.trim().is_empty() {
                    format!("HDMI {}", config.id)
                } else {
                    config.label.clone()
                },
                device: config.device.clone(),
                format: config.format.clone(),
                width: config.width,
                height: config.height,
                fps: config.fps,
                quality: config.quality,
                enabled: config.enabled,
                primary,
            },
            config,
            streamer,
            apply_error: None,
            revision: Arc::new(()),
        }
    }
}

pub fn canonical_device(path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    std::fs::canonicalize(path.as_ref()).unwrap_or_else(|_| path.as_ref().to_path_buf())
}

fn capture_changed(a: &VideoSourceConfig, b: &VideoSourceConfig) -> bool {
    a.device != b.device
        || a.format != b.format
        || a.width != b.width
        || a.height != b.height
        || a.fps != b.fps
}

/// Pure validation also covers configured offline devices, whose capabilities are unknown.
pub fn validate_sources(config: &VideoConfig, devices: &[VideoDeviceInfo]) -> Result<()> {
    let sources = config.effective_sources();
    let mut ids = HashSet::new();
    let mut physical = HashSet::new();
    for source in &sources {
        if source.id.is_empty()
            || source.id.len() > 64
            || !source
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !ids.insert(&source.id)
        {
            return Err(AppError::BadRequest(
                "Source IDs must be unique, 1-64 ASCII letters, digits, '-' or '_'".into(),
            ));
        }
        if source.device.trim().is_empty()
            || (sources.len() > 1 && source.device.eq_ignore_ascii_case("auto"))
        {
            return Err(AppError::BadRequest(
                "Multiple sources require explicit device bindings; auto is not allowed".into(),
            ));
        }
        let path = canonical_device(&source.device);
        let device = devices.iter().find(|d| canonical_device(&d.path) == path);
        // Distinct capture nodes on the same physical card must not bypass deduplication.
        let identity = device
            .filter(|d| !d.bus_info.is_empty())
            .map(|d| format!("bus:{}", d.bus_info))
            .unwrap_or_else(|| format!("path:{}", path.display()));
        if !physical.insert(identity) {
            return Err(AppError::BadRequest(
                "Duplicate physical video device".into(),
            ));
        }
        let format = source
            .format
            .parse::<PixelFormat>()
            .map_err(|_| AppError::BadRequest("Unsupported capture format".into()))?;
        if source.id != config.primary_id()
            && !matches!(
                format,
                PixelFormat::Mjpeg
                    | PixelFormat::Jpeg
                    | PixelFormat::Yuyv
                    | PixelFormat::Yvyu
                    | PixelFormat::Nv12
                    | PixelFormat::Nv16
                    | PixelFormat::Nv24
                    | PixelFormat::Rgb24
                    | PixelFormat::Bgr24
            )
        {
            return Err(AppError::BadRequest(
                "Secondary source format cannot be encoded by the MJPEG pipeline".into(),
            ));
        }
        if !(320..=7680).contains(&source.width)
            || !(240..=4320).contains(&source.height)
            || !(1..=120).contains(&source.fps)
            || !(1..=100).contains(&source.quality)
        {
            return Err(AppError::BadRequest(
                "Invalid source dimensions, FPS or quality".into(),
            ));
        }
        if let Some(device) = device {
            if device.control_mode == VideoControlMode::Configurable {
                let f = device
                    .formats
                    .iter()
                    .find(|f| f.format == format)
                    .ok_or_else(|| {
                        AppError::BadRequest(format!(
                            "Source {} does not support {}",
                            source.id, source.format
                        ))
                    })?;
                if !f.resolutions.is_empty() {
                    let r = f
                        .resolutions
                        .iter()
                        .find(|r| r.width == source.width && r.height == source.height)
                        .ok_or_else(|| {
                            AppError::BadRequest(format!(
                                "Unsupported resolution for source {}",
                                source.id
                            ))
                        })?;
                    if !r.fps.is_empty()
                        && !r
                            .fps
                            .iter()
                            .any(|fps| (*fps - source.fps as f64).abs() < 1.0)
                    {
                        return Err(AppError::BadRequest(format!(
                            "Unsupported FPS for source {}",
                            source.id
                        )));
                    }
                }
            }
        }
    }
    if !ids.contains(&config.primary_id().to_string()) {
        return Err(AppError::BadRequest(
            "primary_source_id must identify a configured source".into(),
        ));
    }
    Ok(())
}

impl VideoStreamManager {
    pub async fn sources(&self) -> Vec<SourceInfo> {
        self.sources
            .read()
            .await
            .iter()
            .map(|s| s.info.clone())
            .collect()
    }

    pub async fn primary_source_id(&self) -> String {
        self.sources
            .read()
            .await
            .iter()
            .find(|s| s.info.primary)
            .map(|s| s.info.id.clone())
            .unwrap_or_else(|| "primary".into())
    }

    pub async fn source_info(&self, id: &str) -> Option<SourceInfo> {
        self.sources
            .read()
            .await
            .iter()
            .find(|s| s.info.id == id)
            .map(|s| s.info.clone())
    }

    pub async fn source_status(
        &self,
        id: &str,
        client_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        let sources = self.sources.read().await;
        let source = sources
            .iter()
            .find(|s| s.info.id == id)
            .ok_or_else(|| AppError::NotFound("Video source not found".into()))?;
        let mut stats = if source.info.primary {
            self.stats().await
        } else {
            source.streamer.stats().await
        };
        if source.info.primary {
            if let Some(events) = self.events.read().await.as_ref() {
                if let Some(SystemEvent::StreamStateChanged { state, reason, .. }) =
                    events.latest_video_stream_state()
                {
                    stats.state = state;
                    stats.reason = reason;
                }
            }
        }
        Ok(serde_json::json!({
            "state": if !source.info.enabled { "disabled" } else if source.apply_error.is_some() { "error" } else { stats.state.as_str() },
            "reason": source.apply_error.clone().or(stats.reason),
            "frames": if source.info.primary && self.current_mode().await == StreamMode::WebRTC {
                self.webrtc_streamer.pipeline_stats().await.map(|s| s.encoded_input_frames).unwrap_or(0)
            } else { source.streamer.capture_frames() },
            "connected": client_id.map(|id| source.streamer.mjpeg_handler().has_client(id)),
        }))
    }

    /// GET callers may start capture but never change capture config or global mode.
    pub async fn source_mjpeg(
        &self,
        id: &str,
        client_id: String,
    ) -> Result<(
        Arc<crate::stream::MjpegStreamHandler>,
        crate::stream::ClientGuard,
    )> {
        let sources = self.sources.read().await;
        let source = sources
            .iter()
            .find(|s| s.info.id == id)
            .ok_or_else(|| AppError::NotFound("Video source not found".into()))?;
        if let Some(error) = &source.apply_error {
            return Err(AppError::ServiceUnavailable(error.clone()));
        }
        if !source.info.enabled
            || self.is_switching()
            || self.is_config_changing()
            || (source.info.primary && self.current_mode().await != StreamMode::Mjpeg)
        {
            return Err(AppError::ServiceUnavailable(
                "Source disabled, changing configuration, or primary requires native WebRTC".into(),
            ));
        }
        source.streamer.start().await?;
        let handler = source.streamer.mjpeg_handler();
        let guard = crate::stream::ClientGuard::new(client_id, handler.clone());
        Ok((handler, guard))
    }

    pub async fn validate_video_config(&self, config: &VideoConfig) -> Result<()> {
        let devices = device::enumerate_devices().unwrap_or_default();
        validate_sources(config, &devices)
    }

    /// Update only affected capture pipelines; labels and ordering do not restart a source.
    pub async fn configure_sources(
        self: &Arc<Self>,
        config: &VideoConfig,
        initializing: bool,
    ) -> Result<()> {
        let _apply = self.sources_apply.lock().await;
        if self.is_switching() {
            return Err(AppError::ServiceUnavailable(
                "Stream mode is changing".into(),
            ));
        }
        self.sources_changing
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let result = self.configure_sources_inner(config, initializing).await;
        self.sources_changing
            .store(false, std::sync::atomic::Ordering::SeqCst);
        result
    }

    async fn configure_sources_inner(
        self: &Arc<Self>,
        config: &VideoConfig,
        initializing: bool,
    ) -> Result<()> {
        let mut registry = self.sources.write().await;
        let next = config.effective_sources();
        self.primary_enabled.store(
            next.iter()
                .find(|s| s.id == config.primary_id())
                .is_some_and(|s| s.enabled),
            std::sync::atomic::Ordering::SeqCst,
        );
        self.webrtc_streamer.set_device_recovery_policy(
            next.iter()
                .find(|s| s.id == config.primary_id())
                .is_some_and(|s| !s.device.eq_ignore_ascii_case("auto")),
        );
        // Release affected owners first: a primary switch may exchange two physical cards.
        for old in registry.iter() {
            let unchanged = next.iter().find(|s| s.id == old.info.id).is_some_and(|s| {
                old.info.primary == (s.id == config.primary_id())
                    && s.enabled == old.config.enabled
                    && !capture_changed(s, &old.config)
            });
            if !unchanged {
                if old.info.primary {
                    self.stop().await?;
                } else {
                    old.streamer.stop().await?;
                }
            }
        }
        let mut replacement = Vec::new();
        let mut failures = Vec::new();
        for source_config in next {
            let primary = source_config.id == config.primary_id();
            let old = registry
                .iter()
                .find(|s| s.info.id == source_config.id && s.info.primary == primary);
            let streamer = if primary {
                self.streamer()
            } else {
                old.map(|s| s.streamer.clone())
                    .unwrap_or_else(Streamer::new)
            };
            streamer.set_device_recovery_policy(!source_config.device.eq_ignore_ascii_case("auto"));
            if let Some(events) = self.events.read().await.clone() {
                if primary {
                    streamer.set_event_bus(events).await;
                } else {
                    streamer
                        .set_source_event_bus(events, source_config.id.clone())
                        .await;
                }
            }
            let changed = initializing
                || old.is_none_or(|s| {
                    !s.config.enabled
                        || capture_changed(&s.config, &source_config)
                        || s.apply_error.is_some()
                });
            // A legacy implicit single source keeps the historical hot-plug contract:
            // no card at startup is a normal state, not a configuration error, and the
            // first successful start must still be reachable once a card appears.
            let implicit_auto =
                config.sources.is_empty() && source_config.device.eq_ignore_ascii_case("auto");
            let mut apply_error = None;
            let quality_changed = old.is_none_or(|s| s.config.quality != source_config.quality);
            if quality_changed {
                streamer.set_jpeg_quality(source_config.quality as u8).await;
            }
            // Disabled sources retain desired state without probing/applying hardware.
            // Re-enabling must apply even when the capture fields did not change.
            if changed && source_config.enabled {
                let format = source_config
                    .format
                    .parse::<PixelFormat>()
                    .map_err(|_| AppError::BadRequest("Invalid capture format".into()))?;
                let resolution = Resolution::new(source_config.width, source_config.height);
                let result = if primary && !initializing {
                    self.apply_video_config(
                        &source_config.device,
                        format,
                        resolution,
                        source_config.fps,
                    )
                    .await
                } else {
                    streamer
                        .apply_video_config(
                            &source_config.device,
                            format,
                            resolution,
                            source_config.fps,
                        )
                        .await
                };
                match result {
                    Ok(()) => {}
                    // `auto` with no camera present is recoverable by later `init_auto`;
                    // recording it as an apply failure would permanently block 503-free
                    // start/snapshot after a capture card is plugged in.
                    Err(AppError::VideoError(message))
                        if implicit_auto && message == NO_AUTO_DEVICE_MESSAGE =>
                    {
                        debug!(
                            "No capture device for implicit auto source; deferring to auto-detect"
                        );
                    }
                    Err(error) => {
                        let message = format!("{}: {}", source_config.id, error);
                        failures.push(message.clone());
                        apply_error = Some(message);
                    }
                }
            }
            if primary {
                self.primary_apply_failed
                    .store(apply_error.is_some(), std::sync::atomic::Ordering::SeqCst);
            }
            if let Some(reason) = &apply_error {
                self.publish_event(SystemEvent::VideoSourceStateChanged {
                    source_id: source_config.id.clone(),
                    state: "error".into(),
                    device: Some(source_config.device.clone()),
                    reason: Some(reason.clone()),
                    next_retry_ms: None,
                })
                .await;
                if primary {
                    self.publish_event(SystemEvent::StreamStateChanged {
                        kind: crate::events::StreamKind::Video,
                        state: "error".into(),
                        device: Some(source_config.device.clone()),
                        reason: Some(reason.clone()),
                        next_retry_ms: None,
                    })
                    .await;
                }
            }
            if !source_config.enabled {
                streamer.stop().await?;
            }
            let mut source = Source::new(source_config, primary, streamer);
            source.apply_error = apply_error;
            replacement.push(source);
        }
        *registry = replacement;
        let sources = registry.iter().map(|s| s.info.clone()).collect();
        drop(registry);
        self.publish_event(SystemEvent::VideoSourcesChanged {
            sources,
            primary_source_id: config.primary_id().into(),
        })
        .await;
        if failures.is_empty() {
            Ok(())
        } else {
            Err(AppError::ServiceUnavailable(format!(
                "Desired video configuration saved; source apply failed: {}",
                failures.join("; ")
            )))
        }
    }

    pub async fn shutdown_sources(&self) {
        for source in self.sources.read().await.iter().filter(|s| !s.info.primary) {
            if let Err(error) = source.streamer.stop().await {
                tracing::warn!(%error, "Failed to stop source");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_sources_defer_apply_until_enabled_and_retry_failures() {
        let manager = VideoStreamManager::with_webrtc_streamer(
            Streamer::new(),
            crate::webrtc::WebRtcStreamer::new(),
        );
        let mut c = config();
        for source in &mut c.sources {
            source.enabled = false;
        }
        manager.configure_sources(&c, true).await.unwrap();
        let streamers: Vec<_> = manager
            .sources
            .read()
            .await
            .iter()
            .map(|s| s.streamer.clone())
            .collect();
        for streamer in &streamers {
            *streamer.test_apply_error.lock() = Some("fixture apply failure".into());
        }
        // Changing desired capture settings while disabled must leave the hook unused.
        for source in &mut c.sources {
            source.fps = 25;
        }
        manager.configure_sources(&c, false).await.unwrap();
        for streamer in &streamers {
            assert!(streamer.test_apply_error.lock().is_some());
            assert!(streamer.current_capture_config().await.0.is_none());
        }
        for source in &mut c.sources {
            source.enabled = true;
        }
        assert!(manager.configure_sources(&c, false).await.is_err());
        assert_eq!(manager.sources().await.len(), 3);
        for source in manager.sources.read().await.iter() {
            assert!(source
                .apply_error
                .as_deref()
                .unwrap()
                .contains("fixture apply failure"));
        }
        assert!(!manager.is_config_changing());
        // Desired configuration survives failure, and the next identical update retries.
        manager.configure_sources(&c, false).await.unwrap();
        for source in manager.sources.read().await.iter() {
            assert!(source.apply_error.is_none());
            let (path, _, _, fps, _) = source.streamer.current_capture_config().await;
            assert_eq!(
                path.unwrap(),
                std::path::PathBuf::from(&source.config.device),
            );
            assert_eq!(fps, 25);
        }
        manager.shutdown_sources().await;
    }

    /// Legacy implicit single-source configs keep their hot-plug contract: booting
    /// without a capture card must not latch `primary_apply_failed`, which would
    /// make every later start/snapshot return 503 even after a card appears.
    #[tokio::test]
    async fn implicit_auto_startup_without_a_card_does_not_poison_primary_apply() {
        let primary = Streamer::new();
        let manager = VideoStreamManager::with_webrtc_streamer(
            primary.clone(),
            crate::webrtc::WebRtcStreamer::new(),
        );
        let video = VideoConfig::default();
        assert!(video.sources.is_empty() && video.device.is_none());
        assert_eq!(
            video.effective_sources()[0].device.as_str(),
            "auto",
            "legacy configs synthesize an implicit auto source"
        );

        *primary.test_device_inventory.lock() = Some(vec![]);
        manager.configure_sources(&video, true).await.unwrap();

        let registry = manager.sources.read().await;
        assert_eq!(registry.len(), 1);
        assert!(
            registry[0].apply_error.is_none(),
            "no card at boot is an offline state, not an apply failure"
        );
        drop(registry);
        // Startup must leave the implicit source startable (503-free) so the
        // first card that is plugged in can be opened by init_auto.
        manager.ensure_primary_enabled().unwrap();
        assert!(!primary.is_config_changing());
        assert_eq!(
            primary.state().await,
            crate::video::StreamerState::Uninitialized
        );
        assert!(
            primary.current_capture_config().await.0.is_none(),
            "no hardware was touched while no card existed"
        );
        // A real apply failure must still latch, so the exemption stays narrow.
        let mut explicit = video.clone();
        explicit.sources = vec![VideoSourceConfig {
            id: "primary".into(),
            device: "auto".into(),
            fps: 25,
            ..Default::default()
        }];
        *primary.test_apply_error.lock() = Some("fixture apply failure".into());
        assert!(manager.configure_sources(&explicit, false).await.is_err());
        assert!(manager.ensure_primary_enabled().is_err());
    }

    fn config() -> VideoConfig {
        let mut c = VideoConfig::default();
        c.sources = (1..=3)
            .map(|i| VideoSourceConfig {
                id: i.to_string(),
                device: format!("/dev/source{i}"),
                ..Default::default()
            })
            .collect();
        c.primary_source_id = Some("1".into());
        c.project_primary();
        c
    }
    #[tokio::test]
    async fn registry_preserves_streamers_on_reorder_and_get_does_not_switch_mode() {
        let primary = Streamer::new();
        let manager = VideoStreamManager::with_webrtc_streamer(
            primary.clone(),
            crate::webrtc::WebRtcStreamer::new(),
        );
        let mut c = config();
        manager.configure_sources(&c, true).await.unwrap();
        let extra = manager.sources.read().await[1].streamer.clone();
        c.sources.reverse();
        c.sources[1].label = "Renamed".into();
        manager.configure_sources(&c, false).await.unwrap();
        let registry = manager.sources.read().await;
        assert!(Arc::ptr_eq(
            &registry.iter().find(|s| s.info.id == "1").unwrap().streamer,
            &primary
        ));
        assert!(Arc::ptr_eq(
            &registry.iter().find(|s| s.info.id == "2").unwrap().streamer,
            &extra
        ));
        drop(registry);
        // init may fail without hardware, but it still selects the requested mode.
        let _ = manager.init_with_mode(StreamMode::WebRTC).await;
        assert!(manager.source_mjpeg("1", "test".into()).await.is_err());
        assert_eq!(manager.current_mode().await, StreamMode::WebRTC);
        manager.shutdown_sources().await;
    }

    #[tokio::test]
    async fn extra_lifecycle_never_publishes_legacy_stream_events() {
        let manager = VideoStreamManager::with_webrtc_streamer(
            Streamer::new(),
            crate::webrtc::WebRtcStreamer::new(),
        );
        let events = Arc::new(crate::events::EventBus::new());
        manager.set_event_bus(events.clone()).await;
        manager.configure_sources(&config(), true).await.unwrap();
        let mut legacy = events.subscribe_topic("stream.*").unwrap();
        let mut source_events = events
            .subscribe_topic("video.source.state_changed")
            .unwrap();
        let extra = manager.sources.read().await[1].streamer.clone();
        extra.stop().await.unwrap();
        assert!(legacy.try_recv().is_err());
        assert!(
            matches!(source_events.recv().await.unwrap(), SystemEvent::VideoSourceStateChanged { source_id, .. } if source_id == "2")
        );
        manager.shutdown_sources().await;
    }

    #[test]
    fn rejects_auto_duplicate_ids_and_physical_aliases() {
        let mut c = config();
        assert!(validate_sources(&c, &[]).is_ok());
        c.sources[1].device = "auto".into();
        assert!(validate_sources(&c, &[]).is_err());
        c = config();
        c.sources[1].id = "1".into();
        assert!(validate_sources(&c, &[]).is_err());
        c = config();
        c.sources[1].device = c.sources[0].device.clone();
        assert!(validate_sources(&c, &[]).is_err());
        c = config();
        c.primary_source_id = Some("missing".into());
        assert!(validate_sources(&c, &[]).is_err());
    }
    #[test]
    fn implicit_and_explicit_single_source_and_offline_are_valid() {
        assert!(validate_sources(&VideoConfig::default(), &[]).is_ok());
        let mut c = config();
        c.sources.truncate(1);
        assert!(validate_sources(&c, &[]).is_ok());
        c.sources[0].enabled = false;
        assert!(validate_sources(&c, &[]).is_ok());
    }
    #[test]
    fn reorder_and_label_edits_do_not_change_capture() {
        let mut c = config();
        let first = c.sources[0].clone();
        c.sources.reverse();
        c.sources[2].label = "New label".into();
        assert!(!capture_changed(&first, &c.sources[2]));
        c.sources[2].width = 1280;
        assert!(capture_changed(&first, &c.sources[2]));
    }
}
