use crate::config::{AppConfig, RtspCodec, StreamMode};
use crate::error::Result;
use crate::video::encoder::registry::VideoEncoderType;
use crate::video::encoder::VideoCodecType;
use crate::video::VideoStreamManager;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct StreamCodecConstraints {
    pub rustdesk_enabled: bool,
    pub rtsp_enabled: bool,
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

impl StreamCodecConstraints {
    pub fn unrestricted() -> Self {
        Self {
            rustdesk_enabled: false,
            rtsp_enabled: false,
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

        if rtsp_enabled {
            let locked_codec = match config.rtsp.codec {
                RtspCodec::H264 => VideoCodecType::H264,
                RtspCodec::H265 => VideoCodecType::H265,
            };
            return Self {
                rustdesk_enabled,
                rtsp_enabled,
                allowed_webrtc_codecs: vec![locked_codec],
                allow_mjpeg: false,
                locked_codec: Some(locked_codec),
                reason: if rustdesk_enabled {
                    format!(
                        "RTSP enabled with codec lock ({:?}) and RustDesk enabled",
                        locked_codec
                    )
                } else {
                    format!("RTSP enabled with codec lock ({:?})", locked_codec)
                },
            };
        }

        if rustdesk_enabled {
            return Self {
                rustdesk_enabled,
                rtsp_enabled,
                allowed_webrtc_codecs: vec![
                    VideoCodecType::H264,
                    VideoCodecType::H265,
                    VideoCodecType::VP8,
                    VideoCodecType::VP9,
                ],
                allow_mjpeg: false,
                locked_codec: None,
                reason: "RustDesk enabled, MJPEG disabled".to_string(),
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
        let current_codec = stream_manager.webrtc_streamer().current_video_codec().await;
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
