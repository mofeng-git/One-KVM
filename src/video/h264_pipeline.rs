//! H264 video encoding pipeline for WebRTC streaming
//!
//! This module provides a complete H264 encoding pipeline that connects:
//! 1. Video capture (YUYV/MJPEG from V4L2)
//! 2. Pixel conversion (YUYV → YUV420P) or JPEG decode
//! 3. H264 encoding (via hwcodec)
//! 4. RTP packetization and WebRTC track output

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch, Mutex};
use tracing::{debug, error, info, warn};

use crate::error::{AppError, Result};
use crate::video::convert::Nv12Converter;
use crate::video::encoder::h264::{H264Config, H264Encoder};
use crate::video::format::{PixelFormat, Resolution};
use crate::webrtc::rtp::{H264VideoTrack, H264VideoTrackConfig};

/// H264 pipeline configuration
#[derive(Debug, Clone)]
pub struct H264PipelineConfig {
    /// Input resolution
    pub resolution: Resolution,
    /// Input pixel format (YUYV, NV12, etc.)
    pub input_format: PixelFormat,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// Target FPS
    pub fps: u32,
    /// GOP size (keyframe interval in frames)
    pub gop_size: u32,
    /// Track ID for WebRTC
    pub track_id: String,
    /// Stream ID for WebRTC
    pub stream_id: String,
}

impl Default for H264PipelineConfig {
    fn default() -> Self {
        Self {
            resolution: Resolution::HD720,
            input_format: PixelFormat::Yuyv,
            bitrate_kbps: 8000,
            fps: 30,
            gop_size: 30,
            track_id: "video0".to_string(),
            stream_id: "one-kvm-stream".to_string(),
        }
    }
}

/// H264 pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct H264PipelineStats {
    /// Total frames captured
    pub frames_captured: u64,
    /// Total frames encoded
    pub frames_encoded: u64,
    /// Frames dropped (encoding too slow)
    pub frames_dropped: u64,
    /// Total bytes encoded
    pub bytes_encoded: u64,
    /// Keyframes encoded
    pub keyframes_encoded: u64,
    /// Average encoding time per frame (ms)
    pub avg_encode_time_ms: f32,
    /// Current encoding FPS
    pub current_fps: f32,
    /// Errors encountered
    pub errors: u64,
}

/// H264 video encoding pipeline
pub struct H264Pipeline {
    config: H264PipelineConfig,
    /// H264 encoder instance
    encoder: Arc<Mutex<Option<H264Encoder>>>,
    /// NV12 converter (for BGR24/RGB24/YUYV → NV12)
    nv12_converter: Arc<Mutex<Option<Nv12Converter>>>,
    /// WebRTC video track
    video_track: Arc<H264VideoTrack>,
    /// Pipeline statistics
    stats: Arc<Mutex<H264PipelineStats>>,
    /// Running state
    running: watch::Sender<bool>,
    /// Encode time accumulator for averaging
    encode_times: Arc<Mutex<Vec<f32>>>,
}

impl H264Pipeline {
    /// Create a new H264 pipeline
    pub fn new(config: H264PipelineConfig) -> Result<Self> {
        info!(
            "Creating H264 pipeline: {}x{} @ {} kbps, {} fps",
            config.resolution.width,
            config.resolution.height,
            config.bitrate_kbps,
            config.fps
        );

        // Determine encoder input format based on pipeline input
        // NV12 is optimal for VAAPI, use it for all formats
        // VAAPI encoders typically only support NV12 input
        let encoder_input_format = crate::video::encoder::h264::H264InputFormat::Nv12;

        // Create H264 encoder with appropriate input format
        let encoder_config = H264Config {
            base: crate::video::encoder::traits::EncoderConfig::h264(
                config.resolution,
                config.bitrate_kbps,
            ),
            bitrate_kbps: config.bitrate_kbps,
            gop_size: config.gop_size,
            fps: config.fps,
            input_format: encoder_input_format,
        };

        let encoder = H264Encoder::new(encoder_config)?;
        info!(
            "H264 encoder created: {} ({}) with {:?} input",
            encoder.codec_name(),
            encoder.encoder_type(),
            encoder_input_format
        );

        // Create NV12 converter based on input format
        // All formats are converted to NV12 for VAAPI encoder
        let nv12_converter = match config.input_format {
            // NV12 input - direct passthrough
            PixelFormat::Nv12 => {
                info!("NV12 input: direct passthrough to encoder");
                None
            }

            // YUYV (4:2:2 packed) → NV12
            PixelFormat::Yuyv => {
                info!("YUYV input: converting to NV12");
                Some(Nv12Converter::yuyv_to_nv12(config.resolution))
            }

            // RGB24 → NV12
            PixelFormat::Rgb24 => {
                info!("RGB24 input: converting to NV12");
                Some(Nv12Converter::rgb24_to_nv12(config.resolution))
            }

            // BGR24 → NV12
            PixelFormat::Bgr24 => {
                info!("BGR24 input: converting to NV12");
                Some(Nv12Converter::bgr24_to_nv12(config.resolution))
            }

            // MJPEG/JPEG input - not supported (requires libjpeg for decoding)
            PixelFormat::Mjpeg | PixelFormat::Jpeg => {
                return Err(AppError::VideoError(
                    "MJPEG input format not supported in this build".to_string()
                ));
            }

            _ => {
                return Err(AppError::VideoError(format!(
                    "Unsupported input format for H264 pipeline: {}",
                    config.input_format
                )));
            }
        };

        // Create WebRTC video track
        let track_config = H264VideoTrackConfig {
            track_id: config.track_id.clone(),
            stream_id: config.stream_id.clone(),
            resolution: config.resolution,
            bitrate_kbps: config.bitrate_kbps,
            fps: config.fps,
            profile_level_id: None, // Let browser negotiate the best profile
        };
        let video_track = Arc::new(H264VideoTrack::new(track_config));

        let (running_tx, _) = watch::channel(false);

        Ok(Self {
            config,
            encoder: Arc::new(Mutex::new(Some(encoder))),
            nv12_converter: Arc::new(Mutex::new(nv12_converter)),
            video_track,
            stats: Arc::new(Mutex::new(H264PipelineStats::default())),
            running: running_tx,
            encode_times: Arc::new(Mutex::new(Vec::with_capacity(100))),
        })
    }

    /// Get the WebRTC video track
    pub fn video_track(&self) -> Arc<H264VideoTrack> {
        self.video_track.clone()
    }

    /// Get current statistics
    pub async fn stats(&self) -> H264PipelineStats {
        self.stats.lock().await.clone()
    }

    /// Check if pipeline is running
    pub fn is_running(&self) -> bool {
        *self.running.borrow()
    }

    /// Start the encoding pipeline
    ///
    /// This starts a background task that receives raw frames from the receiver,
    /// encodes them to H264, and sends them to the WebRTC track.
    pub async fn start(&self, mut frame_rx: broadcast::Receiver<Vec<u8>>) {
        if *self.running.borrow() {
            warn!("H264 pipeline already running");
            return;
        }

        let _ = self.running.send(true);
        info!("Starting H264 pipeline (input format: {})", self.config.input_format);

        let encoder = self.encoder.lock().await.take();
        let nv12_converter = self.nv12_converter.lock().await.take();
        let video_track = self.video_track.clone();
        let stats = self.stats.clone();
        let encode_times = self.encode_times.clone();
        let config = self.config.clone();
        let mut running_rx = self.running.subscribe();

        // Spawn encoding task
        tokio::spawn(async move {
            let mut encoder = match encoder {
                Some(e) => e,
                None => {
                    error!("No encoder available");
                    return;
                }
            };

            let mut nv12_converter = nv12_converter;
            let mut frame_count: u64 = 0;
            let mut last_fps_time = Instant::now();
            let mut fps_frame_count: u64 = 0;

            // Flag for one-time warnings
            let mut size_mismatch_warned = false;

            loop {
                tokio::select! {
                    biased;

                    _ = running_rx.changed() => {
                        if !*running_rx.borrow() {
                            info!("H264 pipeline stopping");
                            break;
                        }
                    }

                    result = frame_rx.recv() => {
                        match result {
                            Ok(raw_frame) => {
                                let start = Instant::now();

                                // Validate frame size for uncompressed formats
                                if let Some(expected_size) = config.input_format.frame_size(config.resolution) {
                                    if raw_frame.len() != expected_size && !size_mismatch_warned {
                                        warn!(
                                            "Frame size mismatch: got {} bytes, expected {} for {} {}x{}",
                                            raw_frame.len(),
                                            expected_size,
                                            config.input_format,
                                            config.resolution.width,
                                            config.resolution.height
                                        );
                                        size_mismatch_warned = true;
                                    }
                                }

                                // Update captured count
                                {
                                    let mut s = stats.lock().await;
                                    s.frames_captured += 1;
                                }

                                // Convert to NV12 for VAAPI encoder
                                // BGR24/RGB24/YUYV -> NV12 (via NV12 converter)
                                // NV12 -> pass through
                                //
                                // Optimized: avoid unnecessary allocations and copies
                                frame_count += 1;
                                fps_frame_count += 1;
                                let pts_ms = (frame_count * 1000 / config.fps as u64) as i64;

                                let encode_result = if let Some(ref mut conv) = nv12_converter {
                                    // BGR24/RGB24/YUYV input - convert to NV12
                                    // Optimized: pass reference directly without copy
                                    match conv.convert(&raw_frame) {
                                        Ok(nv12_data) => encoder.encode_raw(nv12_data, pts_ms),
                                        Err(e) => {
                                            error!("NV12 conversion failed: {}", e);
                                            let mut s = stats.lock().await;
                                            s.errors += 1;
                                            continue;
                                        }
                                    }
                                } else {
                                    // NV12 input - pass reference directly
                                    encoder.encode_raw(&raw_frame, pts_ms)
                                };

                                match encode_result {
                                    Ok(frames) => {
                                        if !frames.is_empty() {
                                            let frame = &frames[0];
                                            let is_keyframe = frame.key == 1;

                                            // Send to WebRTC track
                                            let duration = Duration::from_millis(
                                                1000 / config.fps as u64
                                            );

                                            if let Err(e) = video_track
                                                .write_frame(&frame.data, duration, is_keyframe)
                                                .await
                                            {
                                                error!("Failed to write frame to track: {}", e);
                                                let mut s = stats.lock().await;
                                                s.errors += 1;
                                            } else {
                                                // Update stats
                                                let encode_time = start.elapsed().as_secs_f32() * 1000.0;
                                                let mut s = stats.lock().await;
                                                s.frames_encoded += 1;
                                                s.bytes_encoded += frame.data.len() as u64;
                                                if is_keyframe {
                                                    s.keyframes_encoded += 1;
                                                }

                                                // Update encode time average
                                                let mut times = encode_times.lock().await;
                                                times.push(encode_time);
                                                if times.len() > 100 {
                                                    times.remove(0);
                                                }
                                                if !times.is_empty() {
                                                    s.avg_encode_time_ms =
                                                        times.iter().sum::<f32>() / times.len() as f32;
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Encoding failed: {}", e);
                                        let mut s = stats.lock().await;
                                        s.errors += 1;
                                    }
                                }

                                // Update FPS every second
                                if last_fps_time.elapsed() >= Duration::from_secs(1) {
                                    let mut s = stats.lock().await;
                                    s.current_fps = fps_frame_count as f32
                                        / last_fps_time.elapsed().as_secs_f32();
                                    fps_frame_count = 0;
                                    last_fps_time = Instant::now();
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                let mut s = stats.lock().await;
                                s.frames_dropped += n;
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                info!("Frame channel closed, stopping H264 pipeline");
                                break;
                            }
                        }
                    }
                }
            }

            info!("H264 pipeline task exited");
        });
    }

    /// Stop the encoding pipeline
    pub fn stop(&self) {
        if *self.running.borrow() {
            let _ = self.running.send(false);
            info!("Stopping H264 pipeline");
        }
    }

    /// Request a keyframe (force IDR)
    pub async fn request_keyframe(&self) {
        // Note: hwcodec doesn't support on-demand keyframe requests
        // The encoder will produce keyframes based on GOP size
        debug!("Keyframe requested (will occur at next GOP boundary)");
    }

    /// Update bitrate dynamically
    pub async fn set_bitrate(&self, bitrate_kbps: u32) -> Result<()> {
        if let Some(ref mut encoder) = *self.encoder.lock().await {
            encoder.set_bitrate(bitrate_kbps)?;
            info!("H264 pipeline bitrate updated to {} kbps", bitrate_kbps);
        }
        Ok(())
    }
}

/// Builder for H264 pipeline configuration
pub struct H264PipelineBuilder {
    config: H264PipelineConfig,
}

impl H264PipelineBuilder {
    pub fn new() -> Self {
        Self {
            config: H264PipelineConfig::default(),
        }
    }

    pub fn resolution(mut self, resolution: Resolution) -> Self {
        self.config.resolution = resolution;
        self
    }

    pub fn input_format(mut self, format: PixelFormat) -> Self {
        self.config.input_format = format;
        self
    }

    pub fn bitrate_kbps(mut self, bitrate: u32) -> Self {
        self.config.bitrate_kbps = bitrate;
        self
    }

    pub fn fps(mut self, fps: u32) -> Self {
        self.config.fps = fps;
        self
    }

    pub fn gop_size(mut self, gop: u32) -> Self {
        self.config.gop_size = gop;
        self
    }

    pub fn track_id(mut self, id: &str) -> Self {
        self.config.track_id = id.to_string();
        self
    }

    pub fn stream_id(mut self, id: &str) -> Self {
        self.config.stream_id = id.to_string();
        self
    }

    pub fn build(self) -> Result<H264Pipeline> {
        H264Pipeline::new(self.config)
    }
}

impl Default for H264PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_config_default() {
        let config = H264PipelineConfig::default();
        assert_eq!(config.resolution, Resolution::HD720);
        assert_eq!(config.bitrate_kbps, 8000);
        assert_eq!(config.fps, 30);
        assert_eq!(config.gop_size, 30);
    }

    #[test]
    fn test_pipeline_builder() {
        let builder = H264PipelineBuilder::new()
            .resolution(Resolution::HD1080)
            .bitrate_kbps(4000)
            .fps(60)
            .input_format(PixelFormat::Yuyv);

        assert_eq!(builder.config.resolution, Resolution::HD1080);
        assert_eq!(builder.config.bitrate_kbps, 4000);
        assert_eq!(builder.config.fps, 60);
        assert_eq!(builder.config.input_format, PixelFormat::Yuyv);
    }
}
