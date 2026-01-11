//! MJPEG decoder using RKMPP via hwcodec (FFmpeg RAM).

use hwcodec::ffmpeg::AVPixelFormat;
use hwcodec::ffmpeg_ram::decode::{DecodeContext, Decoder};
use tracing::warn;

use crate::error::{AppError, Result};
use crate::video::convert::Nv12Converter;
use crate::video::format::Resolution;

pub struct MjpegRkmppDecoder {
    decoder: Decoder,
    resolution: Resolution,
    nv16_to_nv12: Option<Nv12Converter>,
    last_pixfmt: Option<AVPixelFormat>,
}

impl MjpegRkmppDecoder {
    pub fn new(resolution: Resolution) -> Result<Self> {
        let ctx = DecodeContext {
            name: "mjpeg_rkmpp".to_string(),
            width: resolution.width as i32,
            height: resolution.height as i32,
            sw_pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
            thread_count: 1,
        };
        let decoder = Decoder::new(ctx).map_err(|_| {
            AppError::VideoError("Failed to create mjpeg_rkmpp decoder".to_string())
        })?;
        Ok(Self {
            decoder,
            resolution,
            nv16_to_nv12: None,
            last_pixfmt: None,
        })
    }

    pub fn decode_to_nv12(&mut self, mjpeg: &[u8]) -> Result<Vec<u8>> {
        let frames = self
            .decoder
            .decode(mjpeg)
            .map_err(|e| AppError::VideoError(format!("mjpeg_rkmpp decode failed: {}", e)))?;
        if frames.is_empty() {
            return Err(AppError::VideoError(
                "mjpeg_rkmpp decode returned no frames".to_string(),
            ));
        }
        if frames.len() > 1 {
            warn!(
                "mjpeg_rkmpp decode returned {} frames, using last",
                frames.len()
            );
        }
        let frame = frames
            .pop()
            .ok_or_else(|| AppError::VideoError("mjpeg_rkmpp decode returned empty".to_string()))?;

        if frame.width as u32 != self.resolution.width
            || frame.height as u32 != self.resolution.height
        {
            warn!(
                "mjpeg_rkmpp output size {}x{} differs from expected {}x{}",
                frame.width, frame.height, self.resolution.width, self.resolution.height
            );
        }

        if let Some(last) = self.last_pixfmt {
            if frame.pixfmt != last {
                warn!(
                    "mjpeg_rkmpp output pixfmt changed from {:?} to {:?}",
                    last, frame.pixfmt
                );
            }
        } else {
            self.last_pixfmt = Some(frame.pixfmt);
        }

        let pixfmt = self.last_pixfmt.unwrap_or(frame.pixfmt);
        match pixfmt {
            AVPixelFormat::AV_PIX_FMT_NV12 => Ok(frame.data),
            AVPixelFormat::AV_PIX_FMT_NV16 => {
                if self.nv16_to_nv12.is_none() {
                    self.nv16_to_nv12 = Some(Nv12Converter::nv16_to_nv12(self.resolution));
                }
                let conv = self.nv16_to_nv12.as_mut().unwrap();
                let nv12 = conv.convert(&frame.data)?;
                Ok(nv12.to_vec())
            }
            other => Err(AppError::VideoError(format!(
                "mjpeg_rkmpp output pixfmt {:?} (expected NV12/NV16)",
                other
            ))),
        }
    }
}
