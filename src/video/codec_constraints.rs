use crate::config::{AppConfig, RtspCodec, StreamMode, VncEncoding};
use crate::error::{AppError, Result};
use crate::rustdesk::config::RustDeskCodec;
use crate::video::codec::registry::VideoEncoderType;
use crate::video::codec::VideoCodecType;
use crate::video::VideoStreamManager;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct StreamCodecConstraints {
    pub rustdesk_enabled: bool,
    pub rtsp_enabled: bool,
    pub vnc_enabled: bool,
    pub allowed_webrtc_codecs: Vec<VideoCodecType>,
    pub allow_mjpeg: bool,
    pub locked_codec: Option<VideoCodecType>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ConstraintEnforcementResult {
    pub changed: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThirdPartyCodecLock {
    H26x(VideoCodecType),
    Mjpeg,
}

impl ThirdPartyCodecLock {
    fn label(self) -> &'static str {
        match self {
            ThirdPartyCodecLock::H26x(codec) => codec_to_id(codec),
            ThirdPartyCodecLock::Mjpeg => "mjpeg",
        }
    }

    fn compatible_with(self, other: Self) -> bool {
        self == other
    }
}

#[derive(Debug, Clone, Copy)]
struct ThirdPartySourceLock {
    source: &'static str,
    lock: ThirdPartyCodecLock,
}

impl StreamCodecConstraints {
    pub fn unrestricted() -> Self {
        Self {
            rustdesk_enabled: false,
            rtsp_enabled: false,
            vnc_enabled: false,
            allowed_webrtc_codecs: vec![
                VideoCodecType::H264,
                VideoCodecType::H265,
                VideoCodecType::VP8,
                VideoCodecType::VP9,
            ],
            allow_mjpeg: true,
            locked_codec: None,
            reason: "No codec lock active".to_string(),
        }
    }

    pub fn from_config(config: &AppConfig) -> Self {
        let rustdesk_enabled = config.rustdesk.enabled;
        let rtsp_enabled = config.rtsp.enabled;
        let vnc_enabled = config.vnc.enabled;

        let locks = third_party_locks(config);
        if let Some(first) = locks.first() {
            let sources = locks
                .iter()
                .map(|item| item.source)
                .collect::<Vec<_>>()
                .join("/");
            let reason = format!(
                "{} enabled with codec lock ({})",
                sources,
                first.lock.label()
            );
            return match first.lock {
                ThirdPartyCodecLock::H26x(codec) => Self {
                    rustdesk_enabled,
                    rtsp_enabled,
                    vnc_enabled,
                    allowed_webrtc_codecs: vec![codec],
                    allow_mjpeg: false,
                    locked_codec: Some(codec),
                    reason,
                },
                ThirdPartyCodecLock::Mjpeg => Self {
                    rustdesk_enabled,
                    rtsp_enabled,
                    vnc_enabled,
                    allowed_webrtc_codecs: vec![],
                    allow_mjpeg: true,
                    locked_codec: None,
                    reason,
                },
            };
        }

        Self::unrestricted()
    }

    pub fn is_mjpeg_allowed(&self) -> bool {
        self.allow_mjpeg
    }

    pub fn is_webrtc_codec_allowed(&self, codec: VideoCodecType) -> bool {
        self.allowed_webrtc_codecs.contains(&codec)
    }

    pub fn preferred_webrtc_codec(&self) -> VideoCodecType {
        if let Some(codec) = self.locked_codec {
            return codec;
        }
        self.allowed_webrtc_codecs
            .first()
            .copied()
            .unwrap_or(VideoCodecType::H264)
    }

    pub fn allowed_codecs_for_api(&self) -> Vec<&'static str> {
        let mut codecs = Vec::new();
        if self.allow_mjpeg {
            codecs.push("mjpeg");
        }
        for codec in &self.allowed_webrtc_codecs {
            codecs.push(codec_to_id(*codec));
        }
        codecs
    }
}

pub fn rustdesk_codec_to_video(codec: RustDeskCodec) -> VideoCodecType {
    match codec {
        RustDeskCodec::H264 => VideoCodecType::H264,
        RustDeskCodec::H265 => VideoCodecType::H265,
    }
}

pub fn rtsp_codec_to_video_codec(codec: RtspCodec) -> VideoCodecType {
    match codec {
        RtspCodec::H264 => VideoCodecType::H264,
        RtspCodec::H265 => VideoCodecType::H265,
    }
}

pub fn vnc_encoding_to_video_codec(encoding: VncEncoding) -> Option<VideoCodecType> {
    match encoding {
        VncEncoding::TightJpeg => None,
        VncEncoding::H264 => Some(VideoCodecType::H264),
    }
}

fn rustdesk_lock(config: &AppConfig) -> Option<ThirdPartySourceLock> {
    if config.rustdesk.enabled {
        return Some(ThirdPartySourceLock {
            source: "RustDesk",
            lock: ThirdPartyCodecLock::H26x(rustdesk_codec_to_video(config.rustdesk.codec)),
        });
    }
    None
}

fn rtsp_lock(config: &AppConfig) -> Option<ThirdPartySourceLock> {
    if config.rtsp.enabled {
        return Some(ThirdPartySourceLock {
            source: "RTSP",
            lock: ThirdPartyCodecLock::H26x(rtsp_codec_to_video_codec(config.rtsp.codec.clone())),
        });
    }
    None
}

fn vnc_lock(config: &AppConfig) -> Option<ThirdPartySourceLock> {
    if config.vnc.enabled {
        let lock = match config.vnc.encoding {
            VncEncoding::TightJpeg => ThirdPartyCodecLock::Mjpeg,
            VncEncoding::H264 => ThirdPartyCodecLock::H26x(VideoCodecType::H264),
        };
        return Some(ThirdPartySourceLock {
            source: "VNC",
            lock,
        });
    }
    None
}

fn third_party_locks(config: &AppConfig) -> Vec<ThirdPartySourceLock> {
    [rustdesk_lock(config), rtsp_lock(config), vnc_lock(config)]
        .into_iter()
        .flatten()
        .collect()
}

pub fn validate_third_party_codec_compatibility(config: &AppConfig) -> Result<()> {
    let locks = third_party_locks(config);
    if let Some(first) = locks.first() {
        for item in locks.iter().skip(1) {
            if !first.lock.compatible_with(item.lock) {
                return Err(AppError::BadRequest(format!(
                    "{} codec {} conflicts with {} codec {}; choose a compatible codec or stop the running service first",
                    item.source,
                    item.lock.label(),
                    first.source,
                    first.lock.label()
                )));
            }
        }
    }

    Ok(())
}

pub async fn enforce_constraints_with_stream_manager(
    stream_manager: &Arc<VideoStreamManager>,
    constraints: &StreamCodecConstraints,
) -> Result<ConstraintEnforcementResult> {
    let current_mode = stream_manager.current_mode().await;

    if current_mode == StreamMode::Mjpeg && !constraints.allow_mjpeg {
        let target_codec = constraints.preferred_webrtc_codec();
        stream_manager.set_video_codec(target_codec).await?;
        let _ = stream_manager
            .switch_mode_transaction(StreamMode::WebRTC)
            .await?;
        return Ok(ConstraintEnforcementResult {
            changed: true,
            message: Some(format!(
                "Auto-switched from MJPEG to {} due to codec lock",
                codec_to_id(target_codec)
            )),
        });
    }

    if current_mode == StreamMode::WebRTC {
        if constraints.allow_mjpeg && constraints.allowed_webrtc_codecs.is_empty() {
            let _ = stream_manager
                .switch_mode_transaction(StreamMode::Mjpeg)
                .await?;
            return Ok(ConstraintEnforcementResult {
                changed: true,
                message: Some("Auto-switched from WebRTC to MJPEG due to codec lock".to_string()),
            });
        }

        let current_codec = stream_manager.current_video_codec().await;
        if !constraints.is_webrtc_codec_allowed(current_codec) {
            let target_codec = constraints.preferred_webrtc_codec();
            stream_manager.set_video_codec(target_codec).await?;
            return Ok(ConstraintEnforcementResult {
                changed: true,
                message: Some(format!(
                    "Auto-switched codec from {} to {} due to codec lock",
                    codec_to_id(current_codec),
                    codec_to_id(target_codec)
                )),
            });
        }
    }

    Ok(ConstraintEnforcementResult {
        changed: false,
        message: None,
    })
}

pub fn codec_to_id(codec: VideoCodecType) -> &'static str {
    match codec {
        VideoCodecType::H264 => "h264",
        VideoCodecType::H265 => "h265",
        VideoCodecType::VP8 => "vp8",
        VideoCodecType::VP9 => "vp9",
    }
}

pub fn encoder_codec_to_id(codec: VideoEncoderType) -> &'static str {
    match codec {
        VideoEncoderType::H264 => "h264",
        VideoEncoderType::H265 => "h265",
        VideoEncoderType::VP8 => "vp8",
        VideoEncoderType::VP9 => "vp9",
    }
}

pub fn video_codec_to_encoder_codec(codec: VideoCodecType) -> VideoEncoderType {
    match codec {
        VideoCodecType::H264 => VideoEncoderType::H264,
        VideoCodecType::H265 => VideoEncoderType::H265,
        VideoCodecType::VP8 => VideoEncoderType::VP8,
        VideoCodecType::VP9 => VideoEncoderType::VP9,
    }
}

pub fn encoder_codec_to_video_codec(codec: VideoEncoderType) -> VideoCodecType {
    match codec {
        VideoEncoderType::H264 => VideoCodecType::H264,
        VideoEncoderType::H265 => VideoCodecType::H265,
        VideoEncoderType::VP8 => VideoCodecType::VP8,
        VideoEncoderType::VP9 => VideoCodecType::VP9,
    }
}
