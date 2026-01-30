//! Encoder registry - Detection and management of available video encoders
//!
//! This module provides:
//! - Automatic detection of available hardware/software encoders
//! - Encoder selection based on format and priority
//! - Global registry for encoder availability queries

use std::collections::HashMap;
use std::sync::OnceLock;
use tracing::{debug, info, warn};

use hwcodec::common::{DataFormat, Quality, RateControl};
use hwcodec::ffmpeg::AVPixelFormat;
use hwcodec::ffmpeg_ram::encode::{EncodeContext, Encoder as HwEncoder};
use hwcodec::ffmpeg_ram::CodecInfo;

/// Video encoder format type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoEncoderType {
    /// H.264/AVC
    H264,
    /// H.265/HEVC
    H265,
    /// VP8
    VP8,
    /// VP9
    VP9,
}

impl VideoEncoderType {
    /// Convert to hwcodec DataFormat
    pub fn to_data_format(&self) -> DataFormat {
        match self {
            VideoEncoderType::H264 => DataFormat::H264,
            VideoEncoderType::H265 => DataFormat::H265,
            VideoEncoderType::VP8 => DataFormat::VP8,
            VideoEncoderType::VP9 => DataFormat::VP9,
        }
    }

    /// Create from hwcodec DataFormat
    pub fn from_data_format(format: DataFormat) -> Option<Self> {
        match format {
            DataFormat::H264 => Some(VideoEncoderType::H264),
            DataFormat::H265 => Some(VideoEncoderType::H265),
            DataFormat::VP8 => Some(VideoEncoderType::VP8),
            DataFormat::VP9 => Some(VideoEncoderType::VP9),
            _ => None,
        }
    }

    /// Get codec name prefix for FFmpeg
    pub fn codec_prefix(&self) -> &'static str {
        match self {
            VideoEncoderType::H264 => "h264",
            VideoEncoderType::H265 => "hevc",
            VideoEncoderType::VP8 => "vp8",
            VideoEncoderType::VP9 => "vp9",
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            VideoEncoderType::H264 => "H.264",
            VideoEncoderType::H265 => "H.265/HEVC",
            VideoEncoderType::VP8 => "VP8",
            VideoEncoderType::VP9 => "VP9",
        }
    }

    /// Check if this format requires hardware-only encoding
    /// H264 supports software fallback, others require hardware
    pub fn hardware_only(&self) -> bool {
        match self {
            VideoEncoderType::H264 => false,
            VideoEncoderType::H265 => true,
            VideoEncoderType::VP8 => true,
            VideoEncoderType::VP9 => true,
        }
    }
}

impl std::fmt::Display for VideoEncoderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Encoder backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncoderBackend {
    /// Intel/AMD/NVIDIA VAAPI (Linux)
    Vaapi,
    /// NVIDIA NVENC
    Nvenc,
    /// Intel Quick Sync Video
    Qsv,
    /// AMD AMF
    Amf,
    /// Rockchip MPP
    Rkmpp,
    /// V4L2 Memory-to-Memory (ARM)
    V4l2m2m,
    /// Software encoding (libx264, libx265, libvpx)
    Software,
}

impl EncoderBackend {
    /// Detect backend from codec name
    pub fn from_codec_name(name: &str) -> Self {
        if name.contains("vaapi") {
            EncoderBackend::Vaapi
        } else if name.contains("nvenc") {
            EncoderBackend::Nvenc
        } else if name.contains("qsv") {
            EncoderBackend::Qsv
        } else if name.contains("amf") {
            EncoderBackend::Amf
        } else if name.contains("rkmpp") {
            EncoderBackend::Rkmpp
        } else if name.contains("v4l2m2m") {
            EncoderBackend::V4l2m2m
        } else {
            EncoderBackend::Software
        }
    }

    /// Check if this is a hardware backend
    pub fn is_hardware(&self) -> bool {
        !matches!(self, EncoderBackend::Software)
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            EncoderBackend::Vaapi => "VAAPI",
            EncoderBackend::Nvenc => "NVENC",
            EncoderBackend::Qsv => "QSV",
            EncoderBackend::Amf => "AMF",
            EncoderBackend::Rkmpp => "RKMPP",
            EncoderBackend::V4l2m2m => "V4L2 M2M",
            EncoderBackend::Software => "Software",
        }
    }

    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "vaapi" => Some(EncoderBackend::Vaapi),
            "nvenc" => Some(EncoderBackend::Nvenc),
            "qsv" => Some(EncoderBackend::Qsv),
            "amf" => Some(EncoderBackend::Amf),
            "rkmpp" => Some(EncoderBackend::Rkmpp),
            "v4l2m2m" | "v4l2" => Some(EncoderBackend::V4l2m2m),
            "software" | "cpu" => Some(EncoderBackend::Software),
            _ => None,
        }
    }
}

impl std::fmt::Display for EncoderBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Information about an available encoder
#[derive(Debug, Clone)]
pub struct AvailableEncoder {
    /// Encoder format type
    pub format: VideoEncoderType,
    /// FFmpeg codec name (e.g., "h264_vaapi", "hevc_nvenc")
    pub codec_name: String,
    /// Backend type
    pub backend: EncoderBackend,
    /// Priority (lower is better)
    pub priority: i32,
    /// Whether this is a hardware encoder
    pub is_hardware: bool,
}

impl AvailableEncoder {
    /// Create from hwcodec CodecInfo
    pub fn from_codec_info(info: &CodecInfo) -> Option<Self> {
        let format = VideoEncoderType::from_data_format(info.format)?;
        let backend = EncoderBackend::from_codec_name(&info.name);
        let is_hardware = backend.is_hardware();

        Some(Self {
            format,
            codec_name: info.name.clone(),
            backend,
            priority: info.priority,
            is_hardware,
        })
    }
}

/// Global encoder registry
///
/// Detects and caches available encoders at startup.
/// Use `EncoderRegistry::global()` to access the singleton instance.
pub struct EncoderRegistry {
    /// Available encoders grouped by format
    encoders: HashMap<VideoEncoderType, Vec<AvailableEncoder>>,
    /// Detection resolution (used for testing)
    detection_resolution: (u32, u32),
}

impl EncoderRegistry {
    /// Get the global registry instance
    ///
    /// The registry is initialized lazily on first access with 1920x1080 detection.
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<EncoderRegistry> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let mut registry = EncoderRegistry::new();
            registry.detect_encoders(1920, 1080);
            registry
        })
    }

    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            encoders: HashMap::new(),
            detection_resolution: (0, 0),
        }
    }

    /// Detect all available encoders
    ///
    /// This queries hwcodec/FFmpeg for available encoders and populates the registry.
    pub fn detect_encoders(&mut self, width: u32, height: u32) {
        info!("Detecting available video encoders at {}x{}", width, height);

        self.encoders.clear();
        self.detection_resolution = (width, height);

        // Create test context for encoder detection
        let ctx = EncodeContext {
            name: String::new(),
            mc_name: None,
            width: width as i32,
            height: height as i32,
            pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
            align: 1,
            fps: 30,
            gop: 30,
            rc: RateControl::RC_CBR,
            quality: Quality::Quality_Default,
            kbs: 2000,
            q: 23,
            thread_count: 1,
        };

        const DETECT_TIMEOUT_MS: u64 = 5000;

        // Get all available encoders from hwcodec with a hard timeout
        let all_encoders = {
            use std::sync::mpsc;
            use std::time::Duration;

            info!("Encoder detection timeout: {}ms", DETECT_TIMEOUT_MS);

            let (tx, rx) = mpsc::channel();
            let ctx_clone = ctx.clone();
            std::thread::spawn(move || {
                let result = HwEncoder::available_encoders(ctx_clone, None);
                let _ = tx.send(result);
            });

            match rx.recv_timeout(Duration::from_millis(DETECT_TIMEOUT_MS)) {
                Ok(encoders) => encoders,
                Err(_) => {
                    warn!(
                        "Encoder detection timed out after {}ms, skipping hardware detection",
                        DETECT_TIMEOUT_MS
                    );
                    Vec::new()
                }
            }
        };

        info!("Found {} encoders from hwcodec", all_encoders.len());

        for codec_info in &all_encoders {
            if let Some(encoder) = AvailableEncoder::from_codec_info(codec_info) {
                debug!(
                    "Detected encoder: {} ({}) - {} priority={}",
                    encoder.codec_name, encoder.format, encoder.backend, encoder.priority
                );

                self.encoders
                    .entry(encoder.format)
                    .or_default()
                    .push(encoder);
            }
        }

        // Sort encoders by priority (lower is better)
        for encoders in self.encoders.values_mut() {
            encoders.sort_by_key(|e| e.priority);
        }

        // Register software encoders as fallback
        info!("Registering software encoders...");
        let software_encoders = [
            (VideoEncoderType::H264, "libx264", 100),
            (VideoEncoderType::H265, "libx265", 100),
            (VideoEncoderType::VP8, "libvpx", 100),
            (VideoEncoderType::VP9, "libvpx-vp9", 100),
        ];

        for (format, codec_name, priority) in software_encoders {
            self.encoders
                .entry(format)
                .or_default()
                .push(AvailableEncoder {
                    format,
                    codec_name: codec_name.to_string(),
                    backend: EncoderBackend::Software,
                    priority,
                    is_hardware: false,
                });

            debug!(
                "Registered software encoder: {} for {} (priority: {})",
                codec_name, format, priority
            );
        }

        // Log summary
        for (format, encoders) in &self.encoders {
            let hw_count = encoders.iter().filter(|e| e.is_hardware).count();
            let sw_count = encoders.len() - hw_count;
            info!(
                "{}: {} encoders ({} hardware, {} software)",
                format,
                encoders.len(),
                hw_count,
                sw_count
            );
        }
    }

    /// Get the best encoder for a format
    ///
    /// # Arguments
    /// * `format` - The video format to encode
    /// * `hardware_only` - If true, only return hardware encoders
    ///
    /// # Returns
    /// The best available encoder, or None if no suitable encoder is found
    pub fn best_encoder(
        &self,
        format: VideoEncoderType,
        hardware_only: bool,
    ) -> Option<&AvailableEncoder> {
        self.encoders.get(&format)?.iter().find(
            |e| {
                if hardware_only {
                    e.is_hardware
                } else {
                    true
                }
            },
        )
    }

    /// Get all encoders for a format
    pub fn encoders_for_format(&self, format: VideoEncoderType) -> &[AvailableEncoder] {
        self.encoders
            .get(&format)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all available formats
    ///
    /// # Arguments
    /// * `hardware_only` - If true, only return formats with hardware encoders
    pub fn available_formats(&self, hardware_only: bool) -> Vec<VideoEncoderType> {
        self.encoders
            .iter()
            .filter(|(_, encoders)| {
                if hardware_only {
                    encoders.iter().any(|e| e.is_hardware)
                } else {
                    !encoders.is_empty()
                }
            })
            .map(|(format, _)| *format)
            .collect()
    }

    /// Check if a format is available
    ///
    /// # Arguments
    /// * `format` - The video format to check
    /// * `hardware_only` - If true, only check for hardware encoders
    pub fn is_format_available(&self, format: VideoEncoderType, hardware_only: bool) -> bool {
        self.best_encoder(format, hardware_only).is_some()
    }

    /// Get available formats for user selection
    ///
    /// Returns formats that are actually usable based on their requirements:
    /// - H264: Available if any encoder exists (hardware or software)
    /// - H265/VP8/VP9: Available only if hardware encoder exists
    pub fn selectable_formats(&self) -> Vec<VideoEncoderType> {
        let mut formats = Vec::new();

        // H264 - supports software fallback
        if self.is_format_available(VideoEncoderType::H264, false) {
            formats.push(VideoEncoderType::H264);
        }

        // H265/VP8/VP9 - hardware only
        for format in [
            VideoEncoderType::H265,
            VideoEncoderType::VP8,
            VideoEncoderType::VP9,
        ] {
            if self.is_format_available(format, true) {
                formats.push(format);
            }
        }

        formats
    }

    /// Get detection resolution
    pub fn detection_resolution(&self) -> (u32, u32) {
        self.detection_resolution
    }

    /// Get all available backend types
    pub fn available_backends(&self) -> Vec<EncoderBackend> {
        use std::collections::HashSet;

        let mut backends = HashSet::new();
        for encoders in self.encoders.values() {
            for encoder in encoders {
                backends.insert(encoder.backend);
            }
        }

        let mut result: Vec<_> = backends.into_iter().collect();
        // Sort: hardware backends first, software last
        result.sort_by_key(|b| if b.is_hardware() { 0 } else { 1 });
        result
    }

    /// Get formats supported by a specific backend
    pub fn formats_for_backend(&self, backend: EncoderBackend) -> Vec<VideoEncoderType> {
        let mut formats = Vec::new();
        for (format, encoders) in &self.encoders {
            if encoders.iter().any(|e| e.backend == backend) {
                formats.push(*format);
            }
        }
        formats
    }

    /// Get encoder for a format with specific backend
    pub fn encoder_with_backend(
        &self,
        format: VideoEncoderType,
        backend: EncoderBackend,
    ) -> Option<&AvailableEncoder> {
        self.encoders
            .get(&format)?
            .iter()
            .find(|e| e.backend == backend)
    }

    /// Get encoders grouped by backend for a format
    pub fn encoders_by_backend(
        &self,
        format: VideoEncoderType,
    ) -> HashMap<EncoderBackend, Vec<&AvailableEncoder>> {
        let mut grouped = HashMap::new();
        if let Some(encoders) = self.encoders.get(&format) {
            for encoder in encoders {
                grouped
                    .entry(encoder.backend)
                    .or_insert_with(Vec::new)
                    .push(encoder);
            }
        }
        grouped
    }
}

impl Default for EncoderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_encoder_type_display() {
        assert_eq!(VideoEncoderType::H264.display_name(), "H.264");
        assert_eq!(VideoEncoderType::H265.display_name(), "H.265/HEVC");
        assert_eq!(VideoEncoderType::VP8.display_name(), "VP8");
        assert_eq!(VideoEncoderType::VP9.display_name(), "VP9");
    }

    #[test]
    fn test_encoder_backend_detection() {
        assert_eq!(
            EncoderBackend::from_codec_name("h264_vaapi"),
            EncoderBackend::Vaapi
        );
        assert_eq!(
            EncoderBackend::from_codec_name("hevc_nvenc"),
            EncoderBackend::Nvenc
        );
        assert_eq!(
            EncoderBackend::from_codec_name("h264_qsv"),
            EncoderBackend::Qsv
        );
        assert_eq!(
            EncoderBackend::from_codec_name("libx264"),
            EncoderBackend::Software
        );
    }

    #[test]
    fn test_hardware_only_requirement() {
        assert!(!VideoEncoderType::H264.hardware_only());
        assert!(VideoEncoderType::H265.hardware_only());
        assert!(VideoEncoderType::VP8.hardware_only());
        assert!(VideoEncoderType::VP9.hardware_only());
    }

    #[test]
    fn test_registry_detection() {
        let mut registry = EncoderRegistry::new();
        registry.detect_encoders(1280, 720);

        // Should have detected at least H264 (software fallback available)
        println!("Available formats: {:?}", registry.available_formats(false));
        println!("Selectable formats: {:?}", registry.selectable_formats());
    }
}
