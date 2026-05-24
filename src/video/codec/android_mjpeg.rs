//! Android FFmpeg/MediaCodec MJPEG decoder glue.

use hwcodec::ffmpeg::AVPixelFormat;
use hwcodec::ffmpeg_ram::decode::{DecodeContext, Decoder};
use tracing::{info, warn};

use crate::error::{AppError, Result};
use crate::video::codec::convert::Nv12Converter;
use crate::video::format::{PixelFormat, Resolution};

pub struct AndroidMediaCodecMjpegDecoder {
    decoder: Decoder,
    resolution: Resolution,
    nv12_converter: Option<Nv12Converter>,
    last_output_format: Option<PixelFormat>,
    pending_frames: u32,
}

impl AndroidMediaCodecMjpegDecoder {
    pub fn new(resolution: Resolution) -> Result<Self> {
        let ctx = DecodeContext {
            name: "mjpeg_mediacodec".to_string(),
            width: resolution.width as i32,
            height: resolution.height as i32,
            sw_pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
            thread_count: 1,
        };
        let decoder = Decoder::new(ctx).map_err(|_| {
            AppError::VideoError("Failed to create FFmpeg mjpeg_mediacodec decoder".to_string())
        })?;
        Ok(Self {
            decoder,
            resolution,
            nv12_converter: None,
            last_output_format: None,
            pending_frames: 0,
        })
    }

    pub fn decode_to_nv12(&mut self, mjpeg: &[u8]) -> Result<Vec<u8>> {
        let frames = match self.decoder.decode(mjpeg) {
            Ok(frames) => frames,
            Err(err) if err == -11 => {
                self.pending_frames += 1;
                if self.pending_frames <= 3 {
                    return Err(AppError::VideoError(
                        "mjpeg_mediacodec decode needs more input".to_string(),
                    ));
                }
                return Err(AppError::VideoError(
                    "mjpeg_mediacodec decoder did not output after 3 frames".to_string(),
                ));
            }
            Err(err) => {
                return Err(AppError::VideoError(format!(
                    "mjpeg_mediacodec decode failed: {err}"
                )));
            }
        };
        if frames.is_empty() {
            self.pending_frames += 1;
            if self.pending_frames <= 3 {
                return Err(AppError::VideoError(
                    "mjpeg_mediacodec decode needs more input".to_string(),
                ));
            }
            return Err(AppError::VideoError(
                "mjpeg_mediacodec decoder did not output after 3 frames".to_string(),
            ));
        }
        self.pending_frames = 0;
        if frames.len() > 1 {
            warn!(
                "mjpeg_mediacodec decode returned {} frames, using last",
                frames.len()
            );
        }

        let frame = frames.pop().ok_or_else(|| {
            AppError::VideoError("mjpeg_mediacodec decode returned empty".to_string())
        })?;

        if frame.width as u32 != self.resolution.width
            || frame.height as u32 != self.resolution.height
        {
            warn!(
                "mjpeg_mediacodec output size {}x{} differs from expected {}x{}",
                frame.width, frame.height, self.resolution.width, self.resolution.height
            );
        }

        let output_format = pixel_format_from_av(frame.pixfmt).ok_or_else(|| {
            AppError::VideoError(format!(
                "mjpeg_mediacodec output pixfmt {:?} is not supported",
                frame.pixfmt
            ))
        })?;

        if self.last_output_format != Some(output_format) {
            info!("mjpeg_mediacodec output format: {}", output_format);
            self.last_output_format = Some(output_format);
        }

        match output_format {
            PixelFormat::Nv12 => Ok(frame.data),
            PixelFormat::Nv21 => {
                let converter = self
                    .nv12_converter
                    .get_or_insert_with(|| Nv12Converter::nv21_to_nv12(self.resolution));
                Ok(converter.convert(&frame.data)?.to_vec())
            }
            PixelFormat::Yuv420 => {
                let converter = self
                    .nv12_converter
                    .get_or_insert_with(|| Nv12Converter::yuv420_to_nv12(self.resolution));
                Ok(converter.convert(&frame.data)?.to_vec())
            }
            other => Err(AppError::VideoError(format!(
                "mjpeg_mediacodec output {} cannot be converted to NV12",
                other
            ))),
        }
    }
}

fn pixel_format_from_av(format: AVPixelFormat) -> Option<PixelFormat> {
    match format {
        AVPixelFormat::AV_PIX_FMT_NV12 => Some(PixelFormat::Nv12),
        AVPixelFormat::AV_PIX_FMT_NV21 => Some(PixelFormat::Nv21),
        AVPixelFormat::AV_PIX_FMT_YUV420P | AVPixelFormat::AV_PIX_FMT_YUVJ420P => {
            Some(PixelFormat::Yuv420)
        }
        _ => None,
    }
}

unsafe impl Send for AndroidMediaCodecMjpegDecoder {}
