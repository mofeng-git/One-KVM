//! Android FFmpeg/MediaCodec encoder glue.

use bytes::Bytes;
use hwcodec::common::{Quality, RateControl};
use hwcodec::ffmpeg::{resolve_pixel_format, AVPixelFormat};
use hwcodec::ffmpeg_ram::encode::{EncodeContext, Encoder as HwEncoder};

use crate::error::{AppError, Result};
use crate::video::format::{PixelFormat, Resolution};

pub struct AndroidMediaCodecH264Encoder {
    inner: HwEncoder,
    resolution: Resolution,
    input_format: PixelFormat,
    bitrate_kbps: u32,
}

impl AndroidMediaCodecH264Encoder {
    pub fn new(
        resolution: Resolution,
        input_format: PixelFormat,
        fps: u32,
        bitrate_kbps: u32,
    ) -> Result<Self> {
        let pixfmt = match input_format {
            PixelFormat::Nv12 => resolve_pixel_format("nv12", AVPixelFormat::AV_PIX_FMT_NV12),
            PixelFormat::Yuv420 => {
                resolve_pixel_format("yuv420p", AVPixelFormat::AV_PIX_FMT_YUV420P)
            }
            other => {
                return Err(AppError::VideoError(format!(
                    "FFmpeg h264_mediacodec accepts NV12/YUV420P memory frames; {other} requires conversion first"
                )))
            }
        };

        let ctx = EncodeContext {
            name: "h264_mediacodec".to_string(),
            mc_name: None,
            width: resolution.width as i32,
            height: resolution.height as i32,
            pixfmt,
            align: 1,
            fps: fps.max(1) as i32,
            gop: fps.max(1) as i32,
            rc: RateControl::RC_CBR,
            quality: Quality::Quality_Low,
            kbs: bitrate_kbps.max(1) as i32,
            q: 23,
            thread_count: 1,
        };

        let inner = HwEncoder::new(ctx).map_err(|_| {
            AppError::VideoError("Failed to create FFmpeg h264_mediacodec encoder".to_string())
        })?;

        Ok(Self {
            inner,
            resolution,
            input_format,
            bitrate_kbps: bitrate_kbps.max(1),
        })
    }

    pub fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<AndroidH264Packet>> {
        let min_len = self
            .input_format
            .frame_size(self.resolution)
            .ok_or_else(|| AppError::VideoError("MediaCodec input must be raw YUV".to_string()))?;
        if data.len() < min_len {
            return Err(AppError::VideoError(format!(
                "MediaCodec {} frame too small: {} < {}",
                self.input_format,
                data.len(),
                min_len
            )));
        }

        let packets = self
            .inner
            .encode_bytes(data, pts_ms)
            .map_err(|err| AppError::VideoError(format!("h264_mediacodec encode failed: {err}")))?;

        Ok(packets
            .into_iter()
            .map(|packet| AndroidH264Packet {
                data: packet.data,
                pts: packet.pts,
                key_frame: packet.key == 1,
            })
            .collect())
    }

    pub fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.inner
            .set_bitrate(bitrate_kbps.max(1) as i32)
            .map_err(|_| AppError::VideoError("Failed to set MediaCodec bitrate".to_string()))?;
        self.bitrate_kbps = bitrate_kbps.max(1);
        Ok(())
    }

    pub fn request_keyframe(&mut self) {
        self.inner.request_keyframe();
    }

    pub fn codec_name(&self) -> &str {
        "h264_mediacodec"
    }

    pub fn input_format(&self) -> PixelFormat {
        self.input_format
    }
}

unsafe impl Send for AndroidMediaCodecH264Encoder {}

#[derive(Debug, Clone)]
pub struct AndroidH264Packet {
    pub data: Bytes,
    pub pts: i64,
    pub key_frame: bool,
}
