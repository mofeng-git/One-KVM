//! One-shot capture at the current source resolution. No resize or browser dependency.
use super::codec::{Encoder, EncoderConfig, JpegEncoder, PixelConverter};
use super::{format::PixelFormat, frame::VideoFrame, VideoStreamManager};
use crate::config::StreamMode;
use crate::error::{AppError, Result};
use bytes::Bytes;
use parking_lot::Mutex;
use std::time::Duration;
use tokio::sync::oneshot;

#[derive(Default)]
pub(crate) struct SnapshotSlot(Mutex<Option<oneshot::Sender<VideoFrame>>>);
impl SnapshotSlot {
    pub fn request(&self) -> Result<oneshot::Receiver<VideoFrame>> {
        let mut slot = self.0.lock();
        if slot.as_ref().is_some_and(|tx| !tx.is_closed()) {
            return Err(AppError::ServiceUnavailable(
                "Snapshot already pending".into(),
            ));
        }
        let (tx, rx) = oneshot::channel();
        *slot = Some(tx);
        Ok(rx)
    }
    pub fn cancel(&self) {
        self.0.lock().take();
    }
    pub fn pending(&self) -> bool {
        self.0.lock().as_ref().is_some_and(|tx| !tx.is_closed())
    }
    pub fn publish(&self, frame: &VideoFrame) {
        let mut slot = self.0.lock();
        if !slot.as_ref().is_some_and(|tx| !tx.is_closed()) {
            *slot = None;
            return;
        }
        // UVC devices can start with both partial JPEGs and structurally valid
        // but noisy JPEGs. Skip the initial capture frames and keep waiting for
        // a valid frame within the existing timeout, without reopening capture.
        if frame.online
            && frame.format.is_compressed()
            && (frame.sequence < 3 || turbojpeg::read_header(frame.data()).is_err())
        {
            return;
        }
        if let Some(tx) = slot.take() {
            let _ = tx.send(frame.clone());
        }
    }
}

pub struct Snapshot {
    pub source_id: String,
    pub jpeg: Bytes,
    pub width: u32,
    pub height: u32,
    pub age_ms: u64,
}

impl VideoStreamManager {
    pub async fn capture_snapshot(&self, quality: Option<u8>) -> Result<Snapshot> {
        self.capture_source_snapshot(None, quality).await
    }

    pub async fn capture_source_snapshot(
        &self,
        source_id: Option<&str>,
        quality: Option<u8>,
    ) -> Result<Snapshot> {
        // Serialize only capture startup with source changes. The registry is
        // needed to clone ownership, never to wait for capture or encoding.
        let startup_guard = self.sources_apply.lock().await;
        let sources = self.sources.read().await;
        let source = sources
            .iter()
            .find(|s| match source_id {
                Some(id) => s.info.id == id,
                None => s.info.primary,
            })
            .ok_or_else(|| AppError::NotFound("Video source not found".into()))?
            .clone();
        drop(sources);
        if let Some(error) = &source.apply_error {
            return Err(AppError::ServiceUnavailable(error.clone()));
        }
        if !source.info.enabled {
            return Err(AppError::ServiceUnavailable(
                "Video source is disabled".into(),
            ));
        }
        let id = source.info.id.clone();
        let generation = source.streamer.capture_generation();

        if self.is_switching() || self.is_config_changing() {
            return Err(AppError::ServiceUnavailable(
                "Video configuration is changing; retry later".into(),
            ));
        }
        if quality.is_some_and(|q| !(1..=100).contains(&q)) {
            return Err(AppError::BadRequest(
                "quality must be between 1 and 100".into(),
            ));
        }
        let capture = async {
            let mode = if source.info.primary {
                self.current_mode().await
            } else {
                StreamMode::Mjpeg
            };
            let frame = match mode {
                StreamMode::Mjpeg => {
                    let streamer = &source.streamer;
                    let handler = streamer.mjpeg_handler();
                    let _guard = crate::stream::ClientGuard::new(
                        format!("snapshot-{}", uuid::Uuid::new_v4()),
                        handler,
                    );
                    let rx = streamer.snapshot_slot.request()?;
                    streamer.start().await?;
                    // Stop/config writers can now cancel the fresh-frame wait.
                    drop(startup_guard);
                    rx.await.map_err(|_| {
                        AppError::ServiceUnavailable("Capture stopped before snapshot".into())
                    })?
                }
                StreamMode::WebRTC => {
                    let pipeline = self.snapshot_pipeline().await?;
                    // Keeps the shared capture pipeline alive without opening another video device.
                    let _consumer = pipeline.subscribe();
                    let rx = pipeline.snapshot_slot.request()?;
                    drop(startup_guard);
                    rx.await.map_err(|_| {
                        AppError::ServiceUnavailable("Capture stopped before snapshot".into())
                    })?
                }
            };
            let quality = quality
                .or_else(|| (!frame.format.is_compressed()).then_some(source.config.quality as u8));
            let mut snapshot = tokio::task::spawn_blocking(move || encode_snapshot(frame, quality))
                .await
                .map_err(|_| AppError::VideoError("Snapshot encoder failed".into()))??;
            snapshot.source_id = id;
            Ok::<Snapshot, AppError>(snapshot)
        };
        let snapshot = tokio::time::timeout(Duration::from_secs(10), capture)
            .await
            .map_err(|_| {
                AppError::ServiceUnavailable("Timed out waiting for a fresh video frame".into())
            })??;
        let sources = self.sources.read().await;
        if self.is_switching()
            || self.is_config_changing()
            || source.streamer.capture_generation() != generation
            || !sources.iter().any(|current| {
                current.info.id == source.info.id
                    && current.info.enabled
                    && current.apply_error.is_none()
                    && std::sync::Arc::ptr_eq(&current.revision, &source.revision)
            })
        {
            return Err(AppError::ServiceUnavailable(
                "Video source changed during snapshot; retry later".into(),
            ));
        }
        Ok(snapshot)
    }
}

fn encode_snapshot(frame: VideoFrame, quality: Option<u8>) -> Result<Snapshot> {
    if !frame.online {
        return Err(AppError::ServiceUnavailable("No video signal".into()));
    }
    let resolution = frame.resolution;
    let age = frame.capture_ts;
    let jpeg = if frame.format.is_compressed() {
        let header = turbojpeg::read_header(frame.data()).map_err(jpeg_error)?;
        // Use dimensions from the actual image, including source-following resolution changes.
        let jpeg = if let Some(quality) = quality {
            let image = turbojpeg::decompress(frame.data(), turbojpeg::PixelFormat::RGB)
                .map_err(jpeg_error)?;
            Bytes::from(
                turbojpeg::compress(image.as_deref(), quality as i32, turbojpeg::Subsamp::Sub2x2)
                    .map_err(jpeg_error)?
                    .to_vec(),
            )
        } else {
            frame.owned_data_bytes()
        };
        return Ok(Snapshot {
            source_id: String::new(),
            jpeg,
            width: header.width as u32,
            height: header.height as u32,
            age_ms: age.elapsed().as_millis() as u64,
        });
    } else {
        let compact = super::pipeline::compact_strided_frame_for_encoder(&frame, frame.data())?;
        let data = compact.as_deref().unwrap_or(frame.data());
        let quality = quality.unwrap_or(80) as u32;
        let mut config = EncoderConfig::jpeg(resolution, quality);
        config.input_format = frame.format;
        let mut encoder = JpegEncoder::new(config)?;
        if encoder.supports_format(frame.format) {
            encoder.encode(data, frame.sequence)?.data
        } else {
            let mut converter = match frame.format {
                PixelFormat::Uyvy => Some(PixelConverter::uyvy_to_yuv420p(resolution)),
                PixelFormat::Nv21 => Some(PixelConverter::nv21_to_yuv420p(resolution)),
                PixelFormat::Yvu420 => Some(PixelConverter::yvu420_to_yuv420p(resolution)),
                PixelFormat::Yuv420 => None,
                other => {
                    return Err(AppError::VideoError(format!(
                        "Snapshot format unsupported: {other}"
                    )))
                }
            };
            let i420 = match converter.as_mut() {
                Some(c) => c.convert(data)?,
                None => data,
            };
            Bytes::from(
                turbojpeg::compress_yuv(
                    turbojpeg::YuvImage {
                        pixels: i420,
                        width: resolution.width as usize,
                        height: resolution.height as usize,
                        align: 1,
                        subsamp: turbojpeg::Subsamp::Sub2x2,
                    },
                    quality as i32,
                )
                .map_err(jpeg_error)?
                .to_vec(),
            )
        }
    };
    Ok(Snapshot {
        source_id: String::new(),
        jpeg,
        width: resolution.width,
        height: resolution.height,
        age_ms: age.elapsed().as_millis() as u64,
    })
}
fn jpeg_error(error: turbojpeg::Error) -> AppError {
    AppError::VideoError(format!("Snapshot JPEG: {error}"))
}

#[cfg(test)]
mod tests {
    use super::super::format::Resolution;
    use super::*;

    async fn pending_snapshot() -> (
        std::sync::Arc<VideoStreamManager>,
        std::sync::Arc<crate::video::Streamer>,
        tokio::task::JoinHandle<Result<Snapshot>>,
    ) {
        let streamer = crate::video::Streamer::new();
        streamer.set_test_streaming().await;
        let manager = VideoStreamManager::with_webrtc_streamer(
            streamer.clone(),
            crate::webrtc::WebRtcStreamer::new(),
        );
        let capture_manager = manager.clone();
        let capture = tokio::spawn(async move { capture_manager.capture_snapshot(None).await });
        tokio::time::timeout(Duration::from_secs(1), async {
            while !streamer.snapshot_slot.pending() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("snapshot must reach the fresh-frame wait");
        (manager, streamer, capture)
    }

    #[tokio::test]
    async fn pending_snapshot_does_not_block_disabling_and_stop_cancellation() {
        let (manager, streamer, capture) = pending_snapshot().await;
        let mut video = crate::config::VideoConfig::default();
        video.sources = video.effective_sources();
        video.sources[0].enabled = false;
        tokio::time::timeout(
            Duration::from_secs(1),
            manager.configure_sources(&video, false),
        )
        .await
        .expect("configuration must acquire the registry while snapshot waits")
        .unwrap();
        assert!(tokio::time::timeout(Duration::from_secs(1), capture)
            .await
            .unwrap()
            .unwrap()
            .is_err());
        assert!(!streamer.snapshot_slot.pending());
    }

    #[tokio::test]
    async fn snapshots_reject_removed_disabled_and_reconfigured_owners_after_delivery() {
        for change in [
            "removed",
            "disabled",
            "reconfigured",
            "same-config-new-owner",
        ] {
            let (manager, streamer, capture) = pending_snapshot().await;
            let mut registry =
                tokio::time::timeout(Duration::from_secs(1), manager.sources.write())
                    .await
                    .expect("fresh-frame wait must release the registry");
            streamer.snapshot_slot.publish(&VideoFrame::from_vec(
                vec![80; 24],
                Resolution::new(4, 2),
                PixelFormat::Rgb24,
                12,
                7,
            ));
            match change {
                "removed" => registry.clear(),
                "disabled" => registry[0].info.enabled = false,
                _ => {
                    let mut config = registry[0].config.clone();
                    if change == "reconfigured" {
                        config.fps = 25;
                    }
                    // Even restoring identical fields cannot resurrect an old request.
                    registry[0] = crate::video::multiview::Source::new(config, true, streamer);
                }
            }
            drop(registry);
            let result = tokio::time::timeout(Duration::from_secs(1), capture)
                .await
                .unwrap()
                .unwrap();
            assert!(
                matches!(result, Err(AppError::ServiceUnavailable(message)) if message.contains("changed during snapshot")),
                "{change}"
            );
        }
    }

    #[tokio::test]
    async fn fresh_snapshot_accepts_the_unchanged_owner() {
        let (_manager, streamer, capture) = pending_snapshot().await;
        streamer.snapshot_slot.publish(&VideoFrame::from_vec(
            vec![80; 24],
            Resolution::new(4, 2),
            PixelFormat::Rgb24,
            12,
            7,
        ));
        let snapshot = tokio::time::timeout(Duration::from_secs(1), capture)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.source_id, "primary");
        assert_eq!((snapshot.width, snapshot.height), (4, 2));
    }

    #[tokio::test]
    async fn snapshot_rejects_a_capture_stopped_after_frame_delivery() {
        let (manager, streamer, capture) = pending_snapshot().await;
        let registry = manager.sources.write().await;
        streamer.snapshot_slot.publish(&VideoFrame::from_vec(
            vec![80; 24],
            Resolution::new(4, 2),
            PixelFormat::Rgb24,
            12,
            7,
        ));
        streamer.stop().await.unwrap();
        drop(registry);
        let result = tokio::time::timeout(Duration::from_secs(1), capture)
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(result, Err(AppError::ServiceUnavailable(message)) if message.contains("changed during snapshot"))
        );
    }
    #[tokio::test]
    async fn source_snapshot_rejects_unknown_disabled_and_stale_cached_extra() {
        let manager = VideoStreamManager::with_webrtc_streamer(
            crate::video::Streamer::new(),
            crate::webrtc::WebRtcStreamer::new(),
        );
        let extra = crate::video::Streamer::new();
        extra.set_strict_device_recovery();
        let mut stale = vec![0; 128];
        stale[..2].copy_from_slice(&[0xff, 0xd8]);
        stale[126..].copy_from_slice(&[0xff, 0xd9]);
        extra.mjpeg_handler().update_frame(VideoFrame::from_vec(
            stale,
            Resolution::new(1920, 1080),
            PixelFormat::Mjpeg,
            0,
            9,
        ));
        assert!(extra
            .mjpeg_handler()
            .current_frame()
            .unwrap()
            .is_valid_jpeg());
        let source = crate::config::VideoSourceConfig {
            id: "extra".into(),
            device: "/dev/offline-kvm-test".into(),
            ..Default::default()
        };
        manager
            .sources
            .write()
            .await
            .push(crate::video::multiview::Source::new(source, false, extra));
        assert!(manager
            .capture_source_snapshot(Some("extra"), None)
            .await
            .is_err());
        assert!(manager
            .capture_source_snapshot(Some("missing"), None)
            .await
            .is_err());
        manager.sources.write().await[1].info.enabled = false;
        assert!(manager
            .capture_source_snapshot(Some("extra"), None)
            .await
            .is_err());
    }

    #[test]
    fn snapshot_preserves_native_resolution_and_padded_rows() {
        let width = 2048;
        let height = 4;
        let stride = width * 3 + 32;
        let frame = VideoFrame::from_vec(
            vec![90; stride * height],
            Resolution::new(width as u32, height as u32),
            PixelFormat::Rgb24,
            stride as u32,
            1,
        );
        let snapshot = encode_snapshot(frame, Some(80)).unwrap();
        let header = turbojpeg::read_header(&snapshot.jpeg).unwrap();
        assert_eq!((header.width, header.height), (width, height));
        assert_eq!(
            (snapshot.width, snapshot.height),
            (width as u32, height as u32)
        );
    }
    #[test]
    fn jpeg_passthrough_and_quality_change_preserve_dimensions() {
        let resolution = Resolution::new(2048, 4);
        let raw = VideoFrame::from_vec(
            vec![80; 2048 * 4 * 3],
            resolution,
            PixelFormat::Rgb24,
            2048 * 3,
            1,
        );
        let first = encode_snapshot(raw, Some(80)).unwrap();
        let source = VideoFrame::new(first.jpeg.clone(), resolution, PixelFormat::Mjpeg, 0, 2);
        let copied = encode_snapshot(source.clone(), None).unwrap();
        assert_eq!(copied.jpeg, first.jpeg);
        let changed = encode_snapshot(source, Some(60)).unwrap();
        let header = turbojpeg::read_header(&changed.jpeg).unwrap();
        assert_eq!((header.width, header.height), (2048, 4));
    }
    #[tokio::test]
    async fn stopping_capture_cancels_the_pending_fresh_request() {
        let slot = SnapshotSlot::default();
        let rx = slot.request().unwrap();
        slot.cancel();
        assert!(rx.await.is_err());
        assert!(!slot.pending());
    }

    #[tokio::test]
    async fn slot_delivers_only_a_requested_new_frame() {
        let slot = SnapshotSlot::default();
        assert!(!slot.pending());
        let rx = slot.request().unwrap();
        assert!(slot.request().is_err());
        let frame = VideoFrame::from_vec(
            vec![0; 24],
            Resolution::new(4, 2),
            PixelFormat::Rgb24,
            12,
            7,
        );
        slot.publish(&frame);
        assert_eq!(rx.await.unwrap().sequence, 7);
        assert!(!slot.pending());
        let abandoned = slot.request().unwrap();
        drop(abandoned);
        assert!(slot.request().is_ok());
    }

    #[tokio::test]
    async fn snapshot_waits_past_invalid_and_unsettled_startup_jpeg() {
        let slot = SnapshotSlot::default();
        let mut rx = slot.request().unwrap();
        let resolution = Resolution::new(4, 2);
        let partial = VideoFrame::from_vec(vec![0; 128], resolution, PixelFormat::Mjpeg, 0, 3);
        slot.publish(&partial);
        assert!(matches!(
            rx.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        assert!(slot.pending());
        let raw = VideoFrame::from_vec(vec![80; 24], resolution, PixelFormat::Rgb24, 12, 2);
        let jpeg = encode_snapshot(raw, None).unwrap();
        let startup = VideoFrame::new(jpeg.jpeg.clone(), resolution, PixelFormat::Mjpeg, 0, 2);
        slot.publish(&startup);
        assert!(matches!(
            rx.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        let complete = VideoFrame::new(jpeg.jpeg, resolution, PixelFormat::Mjpeg, 0, 3);
        slot.publish(&complete);
        assert_eq!(rx.await.unwrap().sequence, 3);
        assert!(!slot.pending());
    }
}
