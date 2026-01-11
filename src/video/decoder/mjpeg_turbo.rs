//! MJPEG decoder using TurboJPEG (software) -> RGB24.

use turbojpeg::{Decompressor, Image, PixelFormat as TJPixelFormat};

use crate::error::{AppError, Result};
use crate::video::format::Resolution;

pub struct MjpegTurboDecoder {
    decompressor: Decompressor,
    resolution: Resolution,
}

impl MjpegTurboDecoder {
    pub fn new(resolution: Resolution) -> Result<Self> {
        let decompressor = Decompressor::new().map_err(|e| {
            AppError::VideoError(format!("Failed to create turbojpeg decoder: {}", e))
        })?;
        Ok(Self {
            decompressor,
            resolution,
        })
    }

    pub fn decode_to_rgb(&mut self, mjpeg: &[u8]) -> Result<Vec<u8>> {
        let header = self
            .decompressor
            .read_header(mjpeg)
            .map_err(|e| AppError::VideoError(format!("turbojpeg read_header failed: {}", e)))?;

        if header.width as u32 != self.resolution.width
            || header.height as u32 != self.resolution.height
        {
            return Err(AppError::VideoError(format!(
                "turbojpeg size mismatch: {}x{} (expected {}x{})",
                header.width, header.height, self.resolution.width, self.resolution.height
            )));
        }

        let pitch = header.width * 3;
        let mut image = Image {
            pixels: vec![0u8; header.height * pitch],
            width: header.width,
            pitch,
            height: header.height,
            format: TJPixelFormat::RGB,
        };

        self.decompressor
            .decompress(mjpeg, image.as_deref_mut())
            .map_err(|e| AppError::VideoError(format!("turbojpeg decode failed: {}", e)))?;

        Ok(image.pixels)
    }
}
