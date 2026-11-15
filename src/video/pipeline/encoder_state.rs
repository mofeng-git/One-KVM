use crate::error::{AppError, Result};
use crate::video::codec::convert::{MjpegToNv12Decoder, Nv12Converter, PixelConverter};
use crate::video::codec::h264::{H264Config, H264Encoder, H264InputFormat};
use crate::video::codec::h265::{H265Config, H265Encoder, H265InputFormat};
use crate::video::codec::registry::{EncoderBackend, EncoderRegistry, VideoEncoderType};
use crate::video::codec::traits::EncoderConfig;
use crate::video::codec::vp8::{VP8Config, VP8Encoder};
use crate::video::codec::vp9::{VP9Config, VP9Encoder};
use crate::video::format::{PixelFormat, Resolution};
use bytes::Bytes;
#[cfg(feature = "rkmpp")]
use hwcodec::ffmpeg_hw::{
    last_error_message as ffmpeg_hw_last_error, HwMjpegH26xConfig, HwMjpegH26xPipeline,
};
#[cfg(all(target_os = "linux", feature = "rkmpp"))]
use hwcodec::rkmpp_native::{
    last_error_message as rkmpp_native_last_error, NativeMppConfig, NativeMppEncoder,
    NativeMppMjpegH264, NativeMppPixelFormat,
};
use tracing::{info, warn};

use super::shared::SharedVideoPipelineConfig;

#[cfg(feature = "rkmpp")]
pub(super) enum HwMjpegPipeline {
    Ffmpeg(HwMjpegH26xPipeline),
    #[cfg(target_os = "linux")]
    NativeMpp(NativeMppMjpegH264),
}

#[cfg(feature = "rkmpp")]
impl HwMjpegPipeline {
    pub(super) fn request_keyframe(&mut self) {
        match self {
            Self::Ffmpeg(p) => p.request_keyframe(),
            #[cfg(target_os = "linux")]
            Self::NativeMpp(_) => {}
        }
    }

    pub(super) fn encode(
        &mut self,
        data: &[u8],
        pts_ms: i64,
        force_idr: bool,
    ) -> std::result::Result<Option<(Vec<u8>, bool)>, String> {
        match self {
            Self::Ffmpeg(p) => {
                if force_idr {
                    p.request_keyframe();
                }
                p.encode(data, pts_ms).map_err(|e| {
                    if e.is_empty() {
                        ffmpeg_hw_last_error()
                    } else {
                        e
                    }
                })
            }
            #[cfg(target_os = "linux")]
            Self::NativeMpp(p) => p.encode(data, pts_ms, force_idr).map_err(|e| {
                if e.is_empty() {
                    rkmpp_native_last_error()
                } else {
                    e
                }
            }),
        }
    }

    pub(super) fn reconfigure(&mut self, bitrate_kbps: i32, gop: i32) -> std::result::Result<(), String> {
        match self {
            Self::Ffmpeg(p) => p.reconfigure(bitrate_kbps, gop).map_err(|e| {
                if e.is_empty() {
                    ffmpeg_hw_last_error()
                } else {
                    e
                }
            }),
            #[cfg(target_os = "linux")]
            Self::NativeMpp(p) => p
                .reconfigure(bitrate_kbps.max(1) as u32, gop.max(1) as u32)
                .map_err(|e| {
                    if e.is_empty() {
                        rkmpp_native_last_error()
                    } else {
                        e
                    }
                }),
        }
    }
}

pub(super) struct EncoderThreadState {
    pub(super) encoder: Option<Box<dyn VideoEncoderTrait + Send>>,
    pub(super) mjpeg_decoder: Option<MjpegDecoderKind>,
    pub(super) nv12_converter: Option<Nv12Converter>,
    pub(super) yuv420p_converter: Option<PixelConverter>,
    pub(super) encoder_needs_yuv420p: bool,
    #[cfg(feature = "rkmpp")]
    pub(super) ffmpeg_hw_pipeline: Option<HwMjpegPipeline>,
    #[cfg(feature = "rkmpp")]
    pub(super) ffmpeg_hw_enabled: bool,
    #[cfg(feature = "rkmpp")]
    pub(super) hw_mjpeg_config: Option<SharedVideoPipelineConfig>,
    pub(super) fps: u32,
    pub(super) codec: VideoEncoderType,
    pub(super) input_format: PixelFormat,
}

pub(super) trait VideoEncoderTrait: Send {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>>;
    fn supports_owned_nv12_input(&self) -> bool {
        false
    }
    fn encode_owned_nv12(&mut self, data: Bytes, pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        self.encode_raw(data.as_ref(), pts_ms)
    }
    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()>;
    fn codec_name(&self) -> &str;
    fn request_keyframe(&mut self);
}

pub(super) struct EncodedFrame {
    pub(super) data: Bytes,
    pub(super) key: i32,
    /// Packet pts in milliseconds, stamped by the encoder when the input
    /// frame was accepted. With a pipelined encoder a packet can be handed
    /// back one call later than its input frame, so consumers must use this
    /// instead of the wall clock time of the current call.
    pub(super) pts: i64,
}

struct H264EncoderWrapper(H264Encoder);

impl VideoEncoderTrait for H264EncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let frames = self.0.encode_raw(data, pts_ms)?;
        Ok(frames
            .into_iter()
            .map(|f| EncodedFrame {
                data: f.data,
                key: f.key,
                pts: f.pts,
            })
            .collect())
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.0.set_bitrate(bitrate_kbps)
    }

    fn supports_owned_nv12_input(&self) -> bool {
        self.0.supports_owned_nv12_input()
    }

    fn encode_owned_nv12(&mut self, data: Bytes, pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        Ok(self
            .0
            .encode_owned_nv12(data, pts_ms)?
            .into_iter()
            .map(|frame| EncodedFrame {
                data: frame.data,
                key: frame.key,
                pts: frame.pts,
            })
            .collect())
    }

    fn codec_name(&self) -> &str {
        self.0.codec_name()
    }

    fn request_keyframe(&mut self) {
        self.0.request_keyframe()
    }
}

#[cfg(all(target_os = "linux", feature = "rkmpp"))]
struct NativeMppEncoderWrapper {
    inner: NativeMppEncoder,
    force_idr: bool,
}

#[cfg(all(target_os = "linux", feature = "rkmpp"))]
impl NativeMppEncoderWrapper {
    fn new(config: &SharedVideoPipelineConfig, input_format: H264InputFormat) -> Result<Self> {
        let fmt = match input_format {
            H264InputFormat::Yuyv422 => NativeMppPixelFormat::Yuyv422,
            H264InputFormat::Yuv420p => NativeMppPixelFormat::Yuv420p,
            _ => NativeMppPixelFormat::Nv12,
        };
        let inner = NativeMppEncoder::new(
            NativeMppConfig {
                width: config.resolution.width,
                height: config.resolution.height,
                fps: config.fps,
                bitrate_kbps: config.bitrate_kbps(),
                gop: config.gop_size().max(1),
            },
            fmt,
        )
        .map_err(|e| AppError::VideoError(format!("Native MPP H264 init failed: {}", e)))?;
        Ok(Self {
            inner,
            force_idr: false,
        })
    }
}

#[cfg(all(target_os = "linux", feature = "rkmpp"))]
impl VideoEncoderTrait for NativeMppEncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let force_idr = std::mem::take(&mut self.force_idr);
        match self.inner.encode(data, pts_ms, force_idr) {
            Ok(Some((packet, is_key))) => Ok(vec![EncodedFrame {
                data: Bytes::from(packet),
                key: if is_key { 1 } else { 0 },
                pts: pts_ms,
            }]),
            Ok(None) => Ok(Vec::new()),
            Err(e) => Err(AppError::VideoError(format!(
                "Native MPP H264 encode failed: {}",
                e
            ))),
        }
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.inner
            .reconfigure(bitrate_kbps, 60)
            .map_err(|e| AppError::VideoError(format!("Native MPP reconfigure failed: {}", e)))
    }

    fn codec_name(&self) -> &str {
        "h264_rkmpp_native"
    }

    fn request_keyframe(&mut self) {
        self.force_idr = true;
    }
}

fn is_rk33xx_soc() -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::sync::OnceLock;
        static IS_RK33XX: OnceLock<bool> = OnceLock::new();
        *IS_RK33XX.get_or_init(|| {
            for path in [
                "/proc/device-tree/compatible",
                "/sys/firmware/devicetree/base/compatible",
            ] {
                if let Ok(bytes) = std::fs::read(path) {
                    if bytes.windows(4).any(|w| w == b"rk33") {
                        return true;
                    }
                }
            }
            false
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[cfg(all(target_os = "linux", feature = "rkmpp"))]
fn to_native_rkmpp_format(input_format: H264InputFormat) -> H264InputFormat {
    match input_format {
        H264InputFormat::Yuyv422 => H264InputFormat::Yuyv422,
        H264InputFormat::Yuv420p => H264InputFormat::Yuv420p,
        _ => H264InputFormat::Nv12,
    }
}

fn create_h264_encoder(
    config: &SharedVideoPipelineConfig,
    input_format: H264InputFormat,
    codec_name: &str,
) -> Result<Box<dyn VideoEncoderTrait + Send>> {
    // On RK33xx (e.g. RK3399 VEPU), FFmpeg's h264_rkmpp passes unaligned ver_stride
    // for 1080p raw frames (0x3f4800 < 0x3fc000). Prefer NativeMppEncoder which
    // allocates 16-aligned planes cleanly unless explicitly forced to ffmpeg mode.
    #[cfg(all(target_os = "linux", feature = "rkmpp"))]
    if codec_name.contains("rkmpp")
        && is_rk33xx_soc()
        && std::env::var("ONE_KVM_RKMPP_MODE").as_deref() != Ok("ffmpeg")
    {
        if let Ok(native) =
            NativeMppEncoderWrapper::new(config, to_native_rkmpp_format(input_format))
        {
            info!(
                "Created H264 encoder: {} (RK33xx aligned native MPP)",
                native.codec_name()
            );
            return Ok(Box::new(native));
        }
    }

    match H264Encoder::with_codec(
        H264Config {
            base: EncoderConfig::h264(config.resolution, config.bitrate_kbps()),
            bitrate_kbps: config.bitrate_kbps(),
            gop_size: config.gop_size(),
            fps: config.fps,
            input_format,
        },
        codec_name,
    ) {
        Ok(encoder) => {
            info!(
                "Created H264 encoder: {} owned_nv12_input={}",
                encoder.codec_name(),
                encoder.supports_owned_nv12_input()
            );
            Ok(Box::new(H264EncoderWrapper(encoder)))
        }
        Err(err) => {
            #[cfg(all(target_os = "linux", feature = "rkmpp"))]
            if codec_name.contains("rkmpp") {
                warn!(
                    "FFmpeg RKMPP encoder ({}) failed ({}), falling back to native MPP C API",
                    codec_name, err
                );
                if let Ok(native) =
                    NativeMppEncoderWrapper::new(config, to_native_rkmpp_format(input_format))
                {
                    info!("Created H264 encoder: {}", native.codec_name());
                    return Ok(Box::new(native));
                }
            }
            Err(err)
        }
    }
}

struct H265EncoderWrapper(H265Encoder);

impl VideoEncoderTrait for H265EncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let frames = self.0.encode_raw(data, pts_ms)?;
        Ok(frames
            .into_iter()
            .map(|f| EncodedFrame {
                data: f.data,
                key: f.key,
                pts: f.pts,
            })
            .collect())
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.0.set_bitrate(bitrate_kbps)
    }

    fn codec_name(&self) -> &str {
        self.0.codec_name()
    }

    fn request_keyframe(&mut self) {
        self.0.request_keyframe()
    }
}

struct VP8EncoderWrapper(VP8Encoder);

impl VideoEncoderTrait for VP8EncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let frames = self.0.encode_raw(data, pts_ms)?;
        Ok(frames
            .into_iter()
            .map(|f| EncodedFrame {
                data: f.data.into(),
                key: f.key,
                pts: f.pts,
            })
            .collect())
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.0.set_bitrate(bitrate_kbps)
    }

    fn codec_name(&self) -> &str {
        self.0.codec_name()
    }

    fn request_keyframe(&mut self) {}
}

struct VP9EncoderWrapper(VP9Encoder);

impl VideoEncoderTrait for VP9EncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let frames = self.0.encode_raw(data, pts_ms)?;
        Ok(frames
            .into_iter()
            .map(|f| EncodedFrame {
                data: f.data.into(),
                key: f.key,
                pts: f.pts,
            })
            .collect())
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.0.set_bitrate(bitrate_kbps)
    }

    fn codec_name(&self) -> &str {
        self.0.codec_name()
    }

    fn request_keyframe(&mut self) {}
}

pub(super) enum MjpegDecoderKind {
    Libyuv { decoder: MjpegToNv12Decoder },
}

impl MjpegDecoderKind {
    pub(super) fn decode(&mut self, data: &[u8]) -> Result<&[u8]> {
        match self {
            MjpegDecoderKind::Libyuv { decoder } => decoder.decode(data),
        }
    }
}

fn libyuv_mjpeg_decoder(resolution: Resolution) -> MjpegDecoderKind {
    MjpegDecoderKind::Libyuv {
        decoder: MjpegToNv12Decoder::new(resolution),
    }
}

fn create_mjpeg_decoder(resolution: Resolution) -> Result<(MjpegDecoderKind, PixelFormat)> {
    info!("MJPEG input detected, using libyuv decoder (MJPEG -> NV12)");
    Ok((libyuv_mjpeg_decoder(resolution), PixelFormat::Nv12))
}

/// V4L2 M2M hardware encoding and libjpeg-turbo use independent CPU/hardware
/// resources. Decode MJPEG outside the encoder worker so encoding the previous
/// NV12 frame can overlap with decoding subsequent frames.
pub(super) fn should_parallel_decode_mjpeg(config: &SharedVideoPipelineConfig) -> bool {
    if !config.input_format.is_compressed()
        || !matches!(
            config.output_codec,
            VideoEncoderType::H264 | VideoEncoderType::H265
        )
    {
        return false;
    }
    let registry = EncoderRegistry::global();
    let selected = match config.encoder_backend {
        Some(backend) => registry.encoder_with_backend(config.output_codec, backend),
        None => registry.best_available_encoder(config.output_codec),
    };
    selected.is_some_and(|encoder| {
        if encoder.backend == EncoderBackend::V4l2m2m {
            return true;
        }
        if encoder.backend == EncoderBackend::Rkmpp {
            return match std::env::var("ONE_KVM_RKMPP_MODE").as_deref() {
                Ok("turbojpeg") => true,
                Ok("native") | Ok("ffmpeg") => false,
                _ => is_rk33xx_soc(),
            };
        }
        false
    })
}

pub(super) fn build_encoder_state(
    config: &SharedVideoPipelineConfig,
) -> Result<EncoderThreadState> {
    let registry = EncoderRegistry::global();

    let get_codec_name =
        |format: VideoEncoderType, backend: Option<EncoderBackend>| -> Option<String> {
            match backend {
                Some(b) => registry
                    .encoder_with_backend(format, b)
                    .map(|e| e.codec_name.clone()),
                None => registry
                    .best_available_encoder(format)
                    .map(|e| e.codec_name.clone()),
            }
        };

    let needs_mjpeg_decode = config.input_format.is_compressed();
    let is_rkmpp_available = registry
        .encoder_with_backend(VideoEncoderType::H264, EncoderBackend::Rkmpp)
        .is_some();
    let rkmpp_is_allowed =
        config.encoder_backend.is_none() || config.encoder_backend == Some(EncoderBackend::Rkmpp);
    let use_yuyv_direct = is_rkmpp_available
        && rkmpp_is_allowed
        && !needs_mjpeg_decode
        && config.input_format == PixelFormat::Yuyv;
    let use_rkmpp_direct = is_rkmpp_available
        && rkmpp_is_allowed
        && !needs_mjpeg_decode
        && matches!(
            config.input_format,
            PixelFormat::Yuyv
                | PixelFormat::Yuv420
                | PixelFormat::Rgb24
                | PixelFormat::Bgr24
                | PixelFormat::Nv12
                | PixelFormat::Nv16
                | PixelFormat::Nv21
                | PixelFormat::Nv24
        );

    if use_yuyv_direct {
        info!("RKMPP backend detected with YUYV input, enabling YUYV direct input optimization");
    } else if use_rkmpp_direct {
        info!(
            "RKMPP backend detected with {} input, enabling direct input optimization",
            config.input_format
        );
    }

    let selected_codec_name = match config.output_codec {
        VideoEncoderType::H264 => {
            if use_rkmpp_direct {
                get_codec_name(VideoEncoderType::H264, Some(EncoderBackend::Rkmpp)).ok_or_else(
                    || AppError::VideoError("RKMPP backend not available for H.264".to_string()),
                )?
            } else if let Some(ref backend) = config.encoder_backend {
                get_codec_name(VideoEncoderType::H264, Some(*backend)).ok_or_else(|| {
                    AppError::VideoError(format!("Backend {:?} does not support H.264", backend))
                })?
            } else {
                get_codec_name(VideoEncoderType::H264, None)
                    .ok_or_else(|| AppError::VideoError("No H.264 encoder available".to_string()))?
            }
        }
        VideoEncoderType::H265 => {
            if use_rkmpp_direct {
                get_codec_name(VideoEncoderType::H265, Some(EncoderBackend::Rkmpp)).ok_or_else(
                    || AppError::VideoError("RKMPP backend not available for H.265".to_string()),
                )?
            } else if let Some(ref backend) = config.encoder_backend {
                get_codec_name(VideoEncoderType::H265, Some(*backend)).ok_or_else(|| {
                    AppError::VideoError(format!("Backend {:?} does not support H.265", backend))
                })?
            } else {
                get_codec_name(VideoEncoderType::H265, None)
                    .ok_or_else(|| AppError::VideoError("No H.265 encoder available".to_string()))?
            }
        }
        VideoEncoderType::VP8 => {
            if let Some(ref backend) = config.encoder_backend {
                get_codec_name(VideoEncoderType::VP8, Some(*backend)).ok_or_else(|| {
                    AppError::VideoError(format!("Backend {:?} does not support VP8", backend))
                })?
            } else {
                get_codec_name(VideoEncoderType::VP8, None)
                    .ok_or_else(|| AppError::VideoError("No VP8 encoder available".to_string()))?
            }
        }
        VideoEncoderType::VP9 => {
            if let Some(ref backend) = config.encoder_backend {
                get_codec_name(VideoEncoderType::VP9, Some(*backend)).ok_or_else(|| {
                    AppError::VideoError(format!("Backend {:?} does not support VP9", backend))
                })?
            } else {
                get_codec_name(VideoEncoderType::VP9, None)
                    .ok_or_else(|| AppError::VideoError("No VP9 encoder available".to_string()))?
            }
        }
    };

    #[cfg(feature = "rkmpp")]
    let is_rkmpp_encoder = selected_codec_name.contains("rkmpp");
    #[cfg(feature = "rkmpp")]
    if needs_mjpeg_decode
        && is_rkmpp_encoder
        && matches!(
            config.output_codec,
            VideoEncoderType::H264 | VideoEncoderType::H265
        )
    {
        let rkmpp_mode = std::env::var("ONE_KVM_RKMPP_MODE").unwrap_or_default();
        if rkmpp_mode != "native" && rkmpp_mode != "turbojpeg" {
            info!(
                "Initializing FFmpeg HW MJPEG->{} pipeline",
                config.output_codec
            );
            match HwMjpegH26xPipeline::new(HwMjpegH26xConfig {
                decoder: "mjpeg_rkmpp".to_string(),
                encoder: selected_codec_name.clone(),
                width: config.resolution.width as i32,
                height: config.resolution.height as i32,
                fps: config.fps as i32,
                bitrate_kbps: config.bitrate_kbps() as i32,
                gop: config.gop_size() as i32,
                thread_count: 1,
            }) {
                Ok(pipeline) => {
                    info!("Using FFmpeg HW MJPEG->{} pipeline", config.output_codec);
                    return Ok(EncoderThreadState {
                        encoder: None,
                        mjpeg_decoder: None,
                        nv12_converter: None,
                        yuv420p_converter: None,
                        encoder_needs_yuv420p: false,
                        ffmpeg_hw_pipeline: Some(HwMjpegPipeline::Ffmpeg(pipeline)),
                        ffmpeg_hw_enabled: true,
                        hw_mjpeg_config: Some(config.clone()),
                        fps: config.fps,
                        codec: config.output_codec,
                        input_format: config.input_format,
                    });
                }
                Err(e) => {
                    let detail = if e.is_empty() {
                        ffmpeg_hw_last_error()
                    } else {
                        e
                    };
                    warn!(
                        "FFmpeg HW MJPEG->{} init failed ({}), trying fallbacks",
                        config.output_codec, detail
                    );
                }
            }
        }

        #[cfg(target_os = "linux")]
        if config.output_codec == VideoEncoderType::H264
            && rkmpp_mode != "ffmpeg"
            && rkmpp_mode != "turbojpeg"
        {
            match NativeMppMjpegH264::new(NativeMppConfig {
                width: config.resolution.width,
                height: config.resolution.height,
                fps: config.fps,
                bitrate_kbps: config.bitrate_kbps(),
                gop: config.gop_size().max(1),
            }) {
                Ok(pipeline) => {
                    info!("Using Native MPP zero-copy MJPEG->H264 pipeline");
                    return Ok(EncoderThreadState {
                        encoder: None,
                        mjpeg_decoder: None,
                        nv12_converter: None,
                        yuv420p_converter: None,
                        encoder_needs_yuv420p: false,
                        ffmpeg_hw_pipeline: Some(HwMjpegPipeline::NativeMpp(pipeline)),
                        ffmpeg_hw_enabled: true,
                        hw_mjpeg_config: Some(config.clone()),
                        fps: config.fps,
                        codec: config.output_codec,
                        input_format: config.input_format,
                    });
                }
                Err(e) => {
                    warn!(
                        "Native MPP MJPEG->H264 init failed ({}), falling back to CPU MJPEG decode",
                        e
                    );
                }
            }
        }
    }

    let (mjpeg_decoder, pipeline_input_format) = if needs_mjpeg_decode {
        let (decoder, format) = create_mjpeg_decoder(config.resolution)?;
        (Some(decoder), format)
    } else {
        (None, config.input_format)
    };

    let encoder: Box<dyn VideoEncoderTrait + Send> = match config.output_codec {
        VideoEncoderType::H264 => {
            let codec_name = selected_codec_name.clone();
            let direct_input_format = h264_direct_input_format(&codec_name, pipeline_input_format);
            let input_format = direct_input_format.unwrap_or_else(|| {
                if codec_name.contains("libx264") {
                    H264InputFormat::Yuv420p
                } else {
                    H264InputFormat::Nv12
                }
            });

            if use_rkmpp_direct {
                info!(
                    "Creating H264 encoder with RKMPP backend for {} direct input (codec: {})",
                    config.input_format, codec_name
                );
            } else if let Some(ref backend) = config.encoder_backend {
                info!(
                    "Creating H264 encoder with backend {:?} (codec: {})",
                    backend, codec_name
                );
            }

            create_h264_encoder(config, input_format, &codec_name)?
        }
        VideoEncoderType::H265 => {
            let codec_name = selected_codec_name.clone();
            let direct_input_format = h265_direct_input_format(&codec_name, pipeline_input_format);
            let input_format = direct_input_format.unwrap_or_else(|| {
                if codec_name.contains("libx265") {
                    H265InputFormat::Yuv420p
                } else {
                    H265InputFormat::Nv12
                }
            });

            if use_rkmpp_direct {
                info!(
                    "Creating H265 encoder with RKMPP backend for {} direct input (codec: {})",
                    config.input_format, codec_name
                );
            } else if let Some(ref backend) = config.encoder_backend {
                info!(
                    "Creating H265 encoder with backend {:?} (codec: {})",
                    backend, codec_name
                );
            }

            let encoder = H265Encoder::with_codec(
                H265Config {
                    base: EncoderConfig {
                        resolution: config.resolution,
                        input_format: config.input_format,
                        quality: config.bitrate_kbps(),
                        fps: config.fps,
                        gop_size: config.gop_size(),
                    },
                    bitrate_kbps: config.bitrate_kbps(),
                    gop_size: config.gop_size(),
                    fps: config.fps,
                    input_format,
                },
                &codec_name,
            )?;
            info!("Created H265 encoder: {}", encoder.codec_name());
            Box::new(H265EncoderWrapper(encoder))
        }
        VideoEncoderType::VP8 => {
            let codec_name = selected_codec_name.clone();
            if let Some(ref backend) = config.encoder_backend {
                info!(
                    "Creating VP8 encoder with backend {:?} (codec: {})",
                    backend, codec_name
                );
            }
            let encoder = VP8Encoder::with_codec(
                VP8Config::low_latency(config.resolution, config.bitrate_kbps()),
                &codec_name,
            )?;
            info!("Created VP8 encoder: {}", encoder.codec_name());
            Box::new(VP8EncoderWrapper(encoder))
        }
        VideoEncoderType::VP9 => {
            let codec_name = selected_codec_name.clone();
            if let Some(ref backend) = config.encoder_backend {
                info!(
                    "Creating VP9 encoder with backend {:?} (codec: {})",
                    backend, codec_name
                );
            }
            let encoder = VP9Encoder::with_codec(
                VP9Config::low_latency(config.resolution, config.bitrate_kbps()),
                &codec_name,
            )?;
            info!("Created VP9 encoder: {}", encoder.codec_name());
            Box::new(VP9EncoderWrapper(encoder))
        }
    };

    let codec_name = encoder.codec_name();
    let use_direct_input = if codec_name.contains("rkmpp") {
        matches!(
            pipeline_input_format,
            PixelFormat::Yuyv
                | PixelFormat::Yuv420
                | PixelFormat::Rgb24
                | PixelFormat::Bgr24
                | PixelFormat::Nv12
                | PixelFormat::Nv16
                | PixelFormat::Nv21
                | PixelFormat::Nv24
        )
    } else if codec_name.contains("libx264") {
        matches!(
            pipeline_input_format,
            PixelFormat::Nv12 | PixelFormat::Nv16 | PixelFormat::Nv21 | PixelFormat::Yuv420
        )
    } else {
        false
    };
    let needs_yuv420p = if codec_name.contains("libx264") {
        !matches!(
            pipeline_input_format,
            PixelFormat::Nv12 | PixelFormat::Nv16 | PixelFormat::Nv21 | PixelFormat::Yuv420
        )
    } else {
        codec_name.contains("libvpx") || codec_name.contains("libx265")
    };

    info!(
        "Encoder {} needs {} format",
        codec_name,
        if use_direct_input {
            "direct"
        } else if needs_yuv420p {
            "YUV420P"
        } else {
            "NV12"
        }
    );
    info!(
        "Initializing input format handler for: {} -> {}",
        pipeline_input_format,
        if use_direct_input {
            "direct"
        } else if needs_yuv420p {
            "YUV420P"
        } else {
            "NV12"
        }
    );

    let (nv12_converter, yuv420p_converter) = converters_for_pipeline(
        config.resolution,
        pipeline_input_format,
        use_yuyv_direct,
        use_direct_input,
        needs_yuv420p,
    )?;

    Ok(EncoderThreadState {
        encoder: Some(encoder),
        mjpeg_decoder,
        nv12_converter,
        yuv420p_converter,
        encoder_needs_yuv420p: needs_yuv420p,
        #[cfg(feature = "rkmpp")]
        ffmpeg_hw_pipeline: None,
        #[cfg(feature = "rkmpp")]
        ffmpeg_hw_enabled: false,
        #[cfg(feature = "rkmpp")]
        hw_mjpeg_config: None,
        fps: config.fps,
        codec: config.output_codec,
        input_format: config.input_format,
    })
}

#[cfg(feature = "rkmpp")]
pub(super) fn fallback_hw_mjpeg_pipeline(
    state: &mut EncoderThreadState,
    reason: &str,
) -> Result<()> {
    let Some(config) = state.hw_mjpeg_config.clone() else {
        return Err(AppError::VideoError(format!(
            "FFmpeg HW encode failed (no fallback config): {}",
            reason
        )));
    };

    let was_ffmpeg = matches!(state.ffmpeg_hw_pipeline, Some(HwMjpegPipeline::Ffmpeg(_)));
    #[cfg(target_os = "linux")]
    if was_ffmpeg
        && config.output_codec == VideoEncoderType::H264
        && std::env::var("ONE_KVM_RKMPP_MODE").as_deref() != Ok("ffmpeg")
    {
        warn!(
            "FFmpeg HW MJPEG->H264 runtime failed ({}), switching to Native MPP zero-copy pipeline",
            reason
        );
        if let Ok(pipeline) = NativeMppMjpegH264::new(NativeMppConfig {
            width: config.resolution.width,
            height: config.resolution.height,
            fps: config.fps,
            bitrate_kbps: config.bitrate_kbps(),
            gop: config.gop_size().max(1),
        }) {
            state.ffmpeg_hw_pipeline = Some(HwMjpegPipeline::NativeMpp(pipeline));
            return Ok(());
        }
    }

    warn!(
        "Hardware MJPEG pipeline failed ({}), falling back to libyuv MJPEG decode + encoder",
        reason
    );
    state.ffmpeg_hw_pipeline = None;
    state.ffmpeg_hw_enabled = false;

    let (mjpeg_decoder, _) = create_mjpeg_decoder(config.resolution)?;
    #[cfg(target_os = "linux")]
    let encoder: Box<dyn VideoEncoderTrait + Send> =
        if config.output_codec == VideoEncoderType::H264 {
            match NativeMppEncoderWrapper::new(&config, H264InputFormat::Nv12) {
                Ok(native) => {
                    info!("Fallback encoder active: {}", native.codec_name());
                    Box::new(native)
                }
                Err(native_err) => {
                    warn!(
                        "Native MPP H264 fallback unavailable ({}), using libx264",
                        native_err
                    );
                    create_h264_encoder(&config, H264InputFormat::Yuv420p, "libx264")?
                }
            }
        } else {
            return Err(AppError::VideoError(format!(
                "No fallback encoder for {}",
                config.output_codec
            )));
        };

    #[cfg(not(target_os = "linux"))]
    let encoder: Box<dyn VideoEncoderTrait + Send> =
        create_h264_encoder(&config, H264InputFormat::Yuv420p, "libx264")?;

    let needs_yuv420p = encoder.codec_name().contains("libx264");
    let (_, yuv420p_converter) = converters_for_pipeline(
        config.resolution,
        PixelFormat::Nv12,
        false,
        !needs_yuv420p,
        needs_yuv420p,
    )?;

    state.mjpeg_decoder = Some(mjpeg_decoder);
    state.encoder = Some(encoder);
    state.nv12_converter = None;
    state.yuv420p_converter = yuv420p_converter;
    state.encoder_needs_yuv420p = needs_yuv420p;
    Ok(())
}

fn h264_direct_input_format(
    codec_name: &str,
    input_format: PixelFormat,
) -> Option<H264InputFormat> {
    if codec_name.contains("rkmpp") {
        match input_format {
            PixelFormat::Yuyv => Some(H264InputFormat::Yuyv422),
            PixelFormat::Yuv420 => Some(H264InputFormat::Yuv420p),
            PixelFormat::Rgb24 => Some(H264InputFormat::Rgb24),
            PixelFormat::Bgr24 => Some(H264InputFormat::Bgr24),
            PixelFormat::Nv12 => Some(H264InputFormat::Nv12),
            PixelFormat::Nv16 => Some(H264InputFormat::Nv16),
            PixelFormat::Nv21 => Some(H264InputFormat::Nv21),
            PixelFormat::Nv24 => Some(H264InputFormat::Nv24),
            _ => None,
        }
    } else if codec_name.contains("libx264") {
        match input_format {
            PixelFormat::Nv12 => Some(H264InputFormat::Nv12),
            PixelFormat::Nv16 => Some(H264InputFormat::Nv16),
            PixelFormat::Nv21 => Some(H264InputFormat::Nv21),
            PixelFormat::Yuv420 => Some(H264InputFormat::Yuv420p),
            _ => None,
        }
    } else {
        None
    }
}

fn h265_direct_input_format(
    codec_name: &str,
    input_format: PixelFormat,
) -> Option<H265InputFormat> {
    if codec_name.contains("rkmpp") {
        match input_format {
            PixelFormat::Yuyv => Some(H265InputFormat::Yuyv422),
            PixelFormat::Yuv420 => Some(H265InputFormat::Yuv420p),
            PixelFormat::Rgb24 => Some(H265InputFormat::Rgb24),
            PixelFormat::Bgr24 => Some(H265InputFormat::Bgr24),
            PixelFormat::Nv12 => Some(H265InputFormat::Nv12),
            PixelFormat::Nv16 => Some(H265InputFormat::Nv16),
            PixelFormat::Nv21 => Some(H265InputFormat::Nv21),
            PixelFormat::Nv24 => Some(H265InputFormat::Nv24),
            _ => None,
        }
    } else if codec_name.contains("libx265") {
        match input_format {
            PixelFormat::Yuv420 => Some(H265InputFormat::Yuv420p),
            _ => None,
        }
    } else {
        None
    }
}

fn converters_for_pipeline(
    resolution: Resolution,
    input_format: PixelFormat,
    use_yuyv_direct: bool,
    use_direct_input: bool,
    needs_yuv420p: bool,
) -> Result<(Option<Nv12Converter>, Option<PixelConverter>)> {
    if use_yuyv_direct {
        info!("YUYV direct input enabled for RKMPP, skipping format conversion");
        return Ok((None, None));
    }
    if use_direct_input {
        info!("Direct input enabled, skipping format conversion");
        return Ok((None, None));
    }
    if needs_yuv420p {
        return match input_format {
            PixelFormat::Yuv420 => {
                info!("Using direct YUV420P input (no conversion)");
                Ok((None, None))
            }
            PixelFormat::Yuyv => {
                info!("Using YUYV->YUV420P converter");
                Ok((None, Some(PixelConverter::yuyv_to_yuv420p(resolution))))
            }
            PixelFormat::Nv12 => {
                info!("Using NV12->YUV420P converter");
                Ok((None, Some(PixelConverter::nv12_to_yuv420p(resolution))))
            }
            PixelFormat::Nv21 => {
                info!("Using NV21->YUV420P converter");
                Ok((None, Some(PixelConverter::nv21_to_yuv420p(resolution))))
            }
            PixelFormat::Nv16 => {
                info!("Using NV16->YUV420P converter");
                Ok((None, Some(PixelConverter::nv16_to_yuv420p(resolution))))
            }
            PixelFormat::Nv24 => {
                info!("Using NV24->YUV420P converter");
                Ok((None, Some(PixelConverter::nv24_to_yuv420p(resolution))))
            }
            PixelFormat::Rgb24 => {
                info!("Using RGB24->YUV420P converter");
                Ok((None, Some(PixelConverter::rgb24_to_yuv420p(resolution))))
            }
            PixelFormat::Bgr24 => {
                info!("Using BGR24->YUV420P converter");
                Ok((None, Some(PixelConverter::bgr24_to_yuv420p(resolution))))
            }
            _ => Err(AppError::VideoError(format!(
                "Unsupported input format for software encoding: {}",
                input_format
            ))),
        };
    }

    match input_format {
        PixelFormat::Nv12 => {
            info!("Using direct NV12 input (no conversion)");
            Ok((None, None))
        }
        PixelFormat::Yuyv => {
            info!("Using YUYV->NV12 converter");
            Ok((Some(Nv12Converter::yuyv_to_nv12(resolution)), None))
        }
        PixelFormat::Nv21 => {
            info!("Using NV21->NV12 converter");
            Ok((Some(Nv12Converter::nv21_to_nv12(resolution)), None))
        }
        PixelFormat::Nv16 => {
            info!("Using NV16->NV12 converter");
            Ok((Some(Nv12Converter::nv16_to_nv12(resolution)), None))
        }
        PixelFormat::Nv24 => {
            info!("Using NV24->NV12 converter");
            Ok((Some(Nv12Converter::nv24_to_nv12(resolution)), None))
        }
        PixelFormat::Yuv420 => {
            info!("Using YUV420P->NV12 converter");
            Ok((Some(Nv12Converter::yuv420_to_nv12(resolution)), None))
        }
        PixelFormat::Rgb24 => {
            info!("Using RGB24->NV12 converter");
            Ok((Some(Nv12Converter::rgb24_to_nv12(resolution)), None))
        }
        PixelFormat::Bgr24 => {
            info!("Using BGR24->NV12 converter");
            Ok((Some(Nv12Converter::bgr24_to_nv12(resolution)), None))
        }
        _ => Err(AppError::VideoError(format!(
            "Unsupported input format for hardware encoding: {}",
            input_format
        ))),
    }
}
