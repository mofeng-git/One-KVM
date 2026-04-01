use crate::error::{AppError, Result};
use crate::video::convert::{Nv12Converter, PixelConverter};
use crate::video::decoder::MjpegTurboDecoder;
use crate::video::encoder::h264::{H264Config, H264Encoder, H264InputFormat};
use crate::video::encoder::h265::{H265Config, H265Encoder, H265InputFormat};
use crate::video::encoder::registry::{EncoderBackend, EncoderRegistry, VideoEncoderType};
use crate::video::encoder::traits::EncoderConfig;
use crate::video::encoder::vp8::{VP8Config, VP8Encoder};
use crate::video::encoder::vp9::{VP9Config, VP9Encoder};
use crate::video::format::{PixelFormat, Resolution};
#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
use hwcodec::ffmpeg_hw::{
    last_error_message as ffmpeg_hw_last_error, HwMjpegH26xConfig, HwMjpegH26xPipeline,
};
use tracing::info;

use super::SharedVideoPipelineConfig;

pub(super) struct EncoderThreadState {
    pub(super) encoder: Option<Box<dyn VideoEncoderTrait + Send>>,
    pub(super) mjpeg_decoder: Option<MjpegDecoderKind>,
    pub(super) nv12_converter: Option<Nv12Converter>,
    pub(super) yuv420p_converter: Option<PixelConverter>,
    pub(super) encoder_needs_yuv420p: bool,
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    pub(super) ffmpeg_hw_pipeline: Option<HwMjpegH26xPipeline>,
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    pub(super) ffmpeg_hw_enabled: bool,
    pub(super) fps: u32,
    pub(super) codec: VideoEncoderType,
    pub(super) input_format: PixelFormat,
}

pub(super) trait VideoEncoderTrait: Send {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>>;
    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()>;
    fn codec_name(&self) -> &str;
    fn request_keyframe(&mut self);
}

pub(super) struct EncodedFrame {
    pub(super) data: Vec<u8>,
    pub(super) key: i32,
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

struct H265EncoderWrapper(H265Encoder);

impl VideoEncoderTrait for H265EncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let frames = self.0.encode_raw(data, pts_ms)?;
        Ok(frames
            .into_iter()
            .map(|f| EncodedFrame {
                data: f.data,
                key: f.key,
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
                data: f.data,
                key: f.key,
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
                data: f.data,
                key: f.key,
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
    Turbo(MjpegTurboDecoder),
}

impl MjpegDecoderKind {
    pub(super) fn decode(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            MjpegDecoderKind::Turbo(decoder) => decoder.decode_to_rgb(data),
        }
    }
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
    let use_yuyv_direct =
        is_rkmpp_available && !needs_mjpeg_decode && config.input_format == PixelFormat::Yuyv;
    let use_rkmpp_direct = is_rkmpp_available
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

    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    let is_rkmpp_encoder = selected_codec_name.contains("rkmpp");
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    if needs_mjpeg_decode
        && is_rkmpp_encoder
        && matches!(
            config.output_codec,
            VideoEncoderType::H264 | VideoEncoderType::H265
        )
    {
        info!(
            "Initializing FFmpeg HW MJPEG->{} pipeline (no fallback)",
            config.output_codec
        );
        let pipeline = HwMjpegH26xPipeline::new(HwMjpegH26xConfig {
            decoder: "mjpeg_rkmpp".to_string(),
            encoder: selected_codec_name.clone(),
            width: config.resolution.width as i32,
            height: config.resolution.height as i32,
            fps: config.fps as i32,
            bitrate_kbps: config.bitrate_kbps() as i32,
            gop: config.gop_size() as i32,
            thread_count: 1,
        })
        .map_err(|e| {
            let detail = if e.is_empty() {
                ffmpeg_hw_last_error()
            } else {
                e
            };
            AppError::VideoError(format!(
                "FFmpeg HW MJPEG->{} init failed: {}",
                config.output_codec, detail
            ))
        })?;
        info!("Using FFmpeg HW MJPEG->{} pipeline", config.output_codec);
        return Ok(EncoderThreadState {
            encoder: None,
            mjpeg_decoder: None,
            nv12_converter: None,
            yuv420p_converter: None,
            encoder_needs_yuv420p: false,
            #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
            ffmpeg_hw_pipeline: Some(pipeline),
            #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
            ffmpeg_hw_enabled: true,
            fps: config.fps,
            codec: config.output_codec,
            input_format: config.input_format,
        });
    }

    let (mjpeg_decoder, pipeline_input_format) = if needs_mjpeg_decode {
        info!(
            "MJPEG input detected, using TurboJPEG decoder ({} -> RGB24)",
            config.input_format
        );
        (
            Some(MjpegDecoderKind::Turbo(MjpegTurboDecoder::new(
                config.resolution,
            )?)),
            PixelFormat::Rgb24,
        )
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

            let encoder = H264Encoder::with_codec(
                H264Config {
                    base: EncoderConfig::h264(config.resolution, config.bitrate_kbps()),
                    bitrate_kbps: config.bitrate_kbps(),
                    gop_size: config.gop_size(),
                    fps: config.fps,
                    input_format,
                },
                &codec_name,
            )?;
            info!("Created H264 encoder: {}", encoder.codec_name());
            Box::new(H264EncoderWrapper(encoder))
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
        #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
        ffmpeg_hw_pipeline: None,
        #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
        ffmpeg_hw_enabled: false,
        fps: config.fps,
        codec: config.output_codec,
        input_format: config.input_format,
    })
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
