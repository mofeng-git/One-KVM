use serde::Serialize;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{
    EncoderRegistry, H264Config, H264Encoder, H265Config, H265Encoder, VP8Config, VP8Encoder,
    VP9Config, VP9Encoder, VideoEncoderType,
};
use crate::error::{AppError, Result};
use crate::video::format::{PixelFormat, Resolution};

const SELF_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const SELF_CHECK_FRAME_ATTEMPTS: u64 = 3;

#[derive(Serialize)]
pub struct VideoEncoderSelfCheckCodec {
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Serialize)]
pub struct VideoEncoderSelfCheckCell {
    pub codec_id: &'static str,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct VideoEncoderSelfCheckRow {
    pub resolution_id: &'static str,
    pub resolution_label: &'static str,
    pub width: u32,
    pub height: u32,
    pub cells: Vec<VideoEncoderSelfCheckCell>,
}

#[derive(Serialize)]
pub struct VideoEncoderSelfCheckResponse {
    pub current_hardware_encoder: String,
    pub codecs: Vec<VideoEncoderSelfCheckCodec>,
    pub rows: Vec<VideoEncoderSelfCheckRow>,
}

pub fn run_hardware_self_check() -> VideoEncoderSelfCheckResponse {
    let registry = EncoderRegistry::global();
    let codecs = codec_columns();
    let mut rows = Vec::new();

    for (resolution_id, resolution_label, resolution) in test_resolutions() {
        let mut cells = Vec::new();

        for codec in test_codecs() {
            let cell = match registry.best_encoder(codec, true) {
                Some(encoder) => run_single_check(codec, resolution, encoder.codec_name.clone()),
                None => unsupported_cell(codec),
            };

            cells.push(cell);
        }

        rows.push(VideoEncoderSelfCheckRow {
            resolution_id,
            resolution_label,
            width: resolution.width,
            height: resolution.height,
            cells,
        });
    }

    VideoEncoderSelfCheckResponse {
        current_hardware_encoder: current_hardware_encoder(registry),
        codecs,
        rows,
    }
}

pub fn build_hardware_self_check_runtime_error() -> VideoEncoderSelfCheckResponse {
    let codecs = codec_columns();
    let mut rows = Vec::new();

    for (resolution_id, resolution_label, resolution) in test_resolutions() {
        let cells = test_codecs()
            .into_iter()
            .map(|codec| VideoEncoderSelfCheckCell {
                codec_id: codec_id(codec),
                ok: false,
                elapsed_ms: None,
            })
            .collect();

        rows.push(VideoEncoderSelfCheckRow {
            resolution_id,
            resolution_label,
            width: resolution.width,
            height: resolution.height,
            cells,
        });
    }

    VideoEncoderSelfCheckResponse {
        current_hardware_encoder: "None".to_string(),
        codecs,
        rows,
    }
}

fn codec_columns() -> Vec<VideoEncoderSelfCheckCodec> {
    test_codecs()
        .into_iter()
        .map(|codec| VideoEncoderSelfCheckCodec {
            id: codec_id(codec),
            name: match codec {
                VideoEncoderType::H265 => "H.265",
                _ => codec.display_name(),
            },
        })
        .collect()
}

fn test_codecs() -> [VideoEncoderType; 4] {
    [
        VideoEncoderType::H264,
        VideoEncoderType::H265,
        VideoEncoderType::VP8,
        VideoEncoderType::VP9,
    ]
}

fn test_resolutions() -> [(&'static str, &'static str, Resolution); 4] {
    [
        ("720p", "720p", Resolution::HD720),
        ("1080p", "1080p", Resolution::HD1080),
        ("2k", "2K", Resolution::new(2560, 1440)),
        ("4k", "4K", Resolution::UHD4K),
    ]
}

fn codec_id(codec: VideoEncoderType) -> &'static str {
    match codec {
        VideoEncoderType::H264 => "h264",
        VideoEncoderType::H265 => "h265",
        VideoEncoderType::VP8 => "vp8",
        VideoEncoderType::VP9 => "vp9",
    }
}

fn unsupported_cell(codec: VideoEncoderType) -> VideoEncoderSelfCheckCell {
    VideoEncoderSelfCheckCell {
        codec_id: codec_id(codec),
        ok: false,
        elapsed_ms: None,
    }
}

fn run_single_check(
    codec: VideoEncoderType,
    resolution: Resolution,
    codec_name_ffmpeg: String,
) -> VideoEncoderSelfCheckCell {
    let started = Instant::now();
    let (tx, rx) = mpsc::channel();
    let thread_codec_name = codec_name_ffmpeg.clone();

    let spawn_result = std::thread::Builder::new()
        .name(format!(
            "encoder-self-check-{}-{}x{}",
            codec_id(codec),
            resolution.width,
            resolution.height
        ))
        .spawn(move || {
            let _ = tx.send(run_smoke_test(codec, resolution, &thread_codec_name));
        });

    if let Err(e) = spawn_result {
        let _ = e;
        return VideoEncoderSelfCheckCell {
            codec_id: codec_id(codec),
            ok: false,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
        };
    }

    match rx.recv_timeout(SELF_CHECK_TIMEOUT) {
        Ok(Ok(())) => VideoEncoderSelfCheckCell {
            codec_id: codec_id(codec),
            ok: true,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
        },
        Ok(Err(_)) => VideoEncoderSelfCheckCell {
            codec_id: codec_id(codec),
            ok: false,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
        },
        Err(mpsc::RecvTimeoutError::Timeout) => VideoEncoderSelfCheckCell {
            codec_id: codec_id(codec),
            ok: false,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
        },
        Err(mpsc::RecvTimeoutError::Disconnected) => VideoEncoderSelfCheckCell {
            codec_id: codec_id(codec),
            ok: false,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
        },
    }
}

fn current_hardware_encoder(registry: &EncoderRegistry) -> String {
    let backends = registry
        .available_backends()
        .into_iter()
        .filter(|backend| backend.is_hardware())
        .map(|backend| backend.display_name().to_string())
        .collect::<Vec<_>>();

    if backends.is_empty() {
        "None".to_string()
    } else {
        backends.join("/")
    }
}

fn run_smoke_test(
    codec: VideoEncoderType,
    resolution: Resolution,
    codec_name_ffmpeg: &str,
) -> Result<()> {
    match codec {
        VideoEncoderType::H264 => run_h264_smoke_test(resolution, codec_name_ffmpeg),
        VideoEncoderType::H265 => run_h265_smoke_test(resolution, codec_name_ffmpeg),
        VideoEncoderType::VP8 => run_vp8_smoke_test(resolution, codec_name_ffmpeg),
        VideoEncoderType::VP9 => run_vp9_smoke_test(resolution, codec_name_ffmpeg),
    }
}

fn run_h264_smoke_test(resolution: Resolution, codec_name_ffmpeg: &str) -> Result<()> {
    let mut encoder = H264Encoder::with_codec(
        H264Config::low_latency(resolution, bitrate_kbps_for_resolution(resolution)),
        codec_name_ffmpeg,
    )?;
    encoder.request_keyframe();
    let frame = build_nv12_test_frame(resolution, encoder.yuv_info().2 as usize);

    for sequence in 0..SELF_CHECK_FRAME_ATTEMPTS {
        let frames = encoder.encode_raw(&frame, pts_ms(sequence))?;
        if frames.iter().any(|frame| !frame.data.is_empty()) {
            return Ok(());
        }
    }

    Err(AppError::VideoError(
        "Encoder produced no output after multiple frames".to_string(),
    ))
}

fn run_h265_smoke_test(resolution: Resolution, codec_name_ffmpeg: &str) -> Result<()> {
    let mut encoder = H265Encoder::with_codec(
        H265Config::low_latency(resolution, bitrate_kbps_for_resolution(resolution)),
        codec_name_ffmpeg,
    )?;
    encoder.request_keyframe();
    let frame = build_nv12_test_frame(resolution, encoder.buffer_info().2 as usize);

    for sequence in 0..SELF_CHECK_FRAME_ATTEMPTS {
        let frames = encoder.encode_raw(&frame, pts_ms(sequence))?;
        if frames.iter().any(|frame| !frame.data.is_empty()) {
            return Ok(());
        }
    }

    Err(AppError::VideoError(
        "Encoder produced no output after multiple frames".to_string(),
    ))
}

fn run_vp8_smoke_test(resolution: Resolution, codec_name_ffmpeg: &str) -> Result<()> {
    let mut encoder = VP8Encoder::with_codec(
        VP8Config::low_latency(resolution, bitrate_kbps_for_resolution(resolution)),
        codec_name_ffmpeg,
    )?;
    let frame = build_nv12_test_frame(resolution, encoder.buffer_info().2 as usize);

    for sequence in 0..SELF_CHECK_FRAME_ATTEMPTS {
        let frames = encoder.encode_raw(&frame, pts_ms(sequence))?;
        if frames.iter().any(|frame| !frame.data.is_empty()) {
            return Ok(());
        }
    }

    Err(AppError::VideoError(
        "Encoder produced no output after multiple frames".to_string(),
    ))
}

fn run_vp9_smoke_test(resolution: Resolution, codec_name_ffmpeg: &str) -> Result<()> {
    let mut encoder = VP9Encoder::with_codec(
        VP9Config::low_latency(resolution, bitrate_kbps_for_resolution(resolution)),
        codec_name_ffmpeg,
    )?;
    let frame = build_nv12_test_frame(resolution, encoder.buffer_info().2 as usize);

    for sequence in 0..SELF_CHECK_FRAME_ATTEMPTS {
        let frames = encoder.encode_raw(&frame, pts_ms(sequence))?;
        if frames.iter().any(|frame| !frame.data.is_empty()) {
            return Ok(());
        }
    }

    Err(AppError::VideoError(
        "Encoder produced no output after multiple frames".to_string(),
    ))
}

fn build_nv12_test_frame(resolution: Resolution, buffer_length: usize) -> Vec<u8> {
    let minimum_length = PixelFormat::Nv12.frame_size(resolution).unwrap_or(0);
    let mut frame = vec![0x80; buffer_length.max(minimum_length)];
    let y_plane_len = (resolution.width * resolution.height) as usize;
    let fill_len = y_plane_len.min(frame.len());
    frame[..fill_len].fill(0x10);
    frame
}

fn bitrate_kbps_for_resolution(resolution: Resolution) -> u32 {
    match resolution.width {
        0..=1280 => 4_000,
        1281..=1920 => 8_000,
        1921..=2560 => 12_000,
        _ => 20_000,
    }
}

fn pts_ms(sequence: u64) -> i64 {
    ((sequence * 1000) / 30) as i64
}
