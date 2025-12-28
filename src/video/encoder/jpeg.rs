//! JPEG encoder implementation
//!
//! Provides JPEG encoding for raw video frames (YUYV, NV12, RGB, BGR)
//! Uses libyuv for SIMD-accelerated color space conversion to I420,
//! then turbojpeg for direct YUV encoding (skips internal color conversion).

use bytes::Bytes;

use super::traits::{EncodedFormat, EncodedFrame, EncoderConfig};
use crate::error::{AppError, Result};
use crate::video::format::{PixelFormat, Resolution};

/// JPEG encoder using libyuv + turbojpeg
///
/// Encoding pipeline (all SIMD accelerated):
/// ```text
/// YUYV/NV12/BGR24/RGB24 ──libyuv──> I420 ──turbojpeg──> JPEG
/// ```
///
/// Note: This encoder is NOT thread-safe due to turbojpeg limitations.
/// Use it from a single thread or wrap in a Mutex.
pub struct JpegEncoder {
    config: EncoderConfig,
    compressor: turbojpeg::Compressor,
    /// I420 buffer for YUV encoding (Y + U + V planes)
    i420_buffer: Vec<u8>,
}

impl JpegEncoder {
    /// Create a new JPEG encoder
    pub fn new(config: EncoderConfig) -> Result<Self> {
        let resolution = config.resolution;
        let width = resolution.width as usize;
        let height = resolution.height as usize;
        // I420: Y = width*height, U = width*height/4, V = width*height/4
        let i420_size = width * height * 3 / 2;

        let mut compressor = turbojpeg::Compressor::new()
            .map_err(|e| AppError::VideoError(format!("Failed to create turbojpeg compressor: {}", e)))?;

        compressor.set_quality(config.quality.min(100) as i32)
            .map_err(|e| AppError::VideoError(format!("Failed to set JPEG quality: {}", e)))?;

        Ok(Self {
            config,
            compressor,
            i420_buffer: vec![0u8; i420_size],
        })
    }

    /// Create with specific quality
    pub fn with_quality(resolution: Resolution, quality: u32) -> Result<Self> {
        let config = EncoderConfig::jpeg(resolution, quality);
        Self::new(config)
    }

    /// Set JPEG quality (1-100)
    pub fn set_quality(&mut self, quality: u32) -> Result<()> {
        self.compressor.set_quality(quality.min(100) as i32)
            .map_err(|e| AppError::VideoError(format!("Failed to set JPEG quality: {}", e)))?;
        self.config.quality = quality;
        Ok(())
    }

    /// Encode I420 buffer to JPEG using turbojpeg's YUV encoder
    #[inline]
    fn encode_i420_to_jpeg(&mut self, sequence: u64) -> Result<EncodedFrame> {
        let width = self.config.resolution.width as usize;
        let height = self.config.resolution.height as usize;

        // Create YuvImage for turbojpeg (I420 = YUV420 = Sub2x2)
        let yuv_image = turbojpeg::YuvImage {
            pixels: self.i420_buffer.as_slice(),
            width,
            height,
            align: 1, // No padding between rows
            subsamp: turbojpeg::Subsamp::Sub2x2, // YUV 4:2:0
        };

        // Compress YUV directly to JPEG (skips color space conversion!)
        let jpeg_data = self.compressor.compress_yuv_to_vec(yuv_image)
            .map_err(|e| AppError::VideoError(format!("JPEG compression failed: {}", e)))?;

        Ok(EncodedFrame::jpeg(
            Bytes::from(jpeg_data),
            self.config.resolution,
            sequence,
        ))
    }

    /// Encode YUYV (YUV422) frame to JPEG
    pub fn encode_yuyv(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        let width = self.config.resolution.width as usize;
        let height = self.config.resolution.height as usize;
        let expected_size = width * height * 2;

        if data.len() < expected_size {
            return Err(AppError::VideoError(format!(
                "YUYV data too small: {} < {}",
                data.len(),
                expected_size
            )));
        }

        // Convert YUYV to I420 using libyuv (SIMD accelerated)
        libyuv::yuy2_to_i420(data, &mut self.i420_buffer, width as i32, height as i32)
            .map_err(|e| AppError::VideoError(format!("libyuv YUYV→I420 failed: {}", e)))?;

        self.encode_i420_to_jpeg(sequence)
    }

    /// Encode NV12 frame to JPEG
    pub fn encode_nv12(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        let width = self.config.resolution.width as usize;
        let height = self.config.resolution.height as usize;
        let expected_size = width * height * 3 / 2;

        if data.len() < expected_size {
            return Err(AppError::VideoError(format!(
                "NV12 data too small: {} < {}",
                data.len(),
                expected_size
            )));
        }

        // Convert NV12 to I420 using libyuv (SIMD accelerated)
        libyuv::nv12_to_i420(data, &mut self.i420_buffer, width as i32, height as i32)
            .map_err(|e| AppError::VideoError(format!("libyuv NV12→I420 failed: {}", e)))?;

        self.encode_i420_to_jpeg(sequence)
    }

    /// Encode RGB24 frame to JPEG
    pub fn encode_rgb(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        let width = self.config.resolution.width as usize;
        let height = self.config.resolution.height as usize;
        let expected_size = width * height * 3;

        if data.len() < expected_size {
            return Err(AppError::VideoError(format!(
                "RGB data too small: {} < {}",
                data.len(),
                expected_size
            )));
        }

        // Convert RGB24 to I420 using libyuv (SIMD accelerated)
        libyuv::rgb24_to_i420(data, &mut self.i420_buffer, width as i32, height as i32)
            .map_err(|e| AppError::VideoError(format!("libyuv RGB24→I420 failed: {}", e)))?;

        self.encode_i420_to_jpeg(sequence)
    }

    /// Encode BGR24 frame to JPEG
    pub fn encode_bgr(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        let width = self.config.resolution.width as usize;
        let height = self.config.resolution.height as usize;
        let expected_size = width * height * 3;

        if data.len() < expected_size {
            return Err(AppError::VideoError(format!(
                "BGR data too small: {} < {}",
                data.len(),
                expected_size
            )));
        }

        // Convert BGR24 to I420 using libyuv (SIMD accelerated)
        // Note: libyuv's RAWToI420 is BGR24 → I420
        libyuv::bgr24_to_i420(data, &mut self.i420_buffer, width as i32, height as i32)
            .map_err(|e| AppError::VideoError(format!("libyuv BGR24→I420 failed: {}", e)))?;

        self.encode_i420_to_jpeg(sequence)
    }
}

impl crate::video::encoder::traits::Encoder for JpegEncoder {
    fn name(&self) -> &str {
        "JPEG (libyuv+turbojpeg)"
    }

    fn output_format(&self) -> EncodedFormat {
        EncodedFormat::Jpeg
    }

    fn encode(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        match self.config.input_format {
            PixelFormat::Yuyv | PixelFormat::Yvyu => self.encode_yuyv(data, sequence),
            PixelFormat::Nv12 => self.encode_nv12(data, sequence),
            PixelFormat::Rgb24 => self.encode_rgb(data, sequence),
            PixelFormat::Bgr24 => self.encode_bgr(data, sequence),
            _ => Err(AppError::VideoError(format!(
                "Unsupported input format for JPEG: {}",
                self.config.input_format
            ))),
        }
    }

    fn config(&self) -> &EncoderConfig {
        &self.config
    }

    fn supports_format(&self, format: PixelFormat) -> bool {
        matches!(
            format,
            PixelFormat::Yuyv
                | PixelFormat::Yvyu
                | PixelFormat::Nv12
                | PixelFormat::Rgb24
                | PixelFormat::Bgr24
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i420_buffer_size() {
        // 1920x1080 I420 = 1920*1080 + 960*540 + 960*540 = 3110400 bytes
        let config = EncoderConfig::jpeg(Resolution::HD1080, 80);
        let encoder = JpegEncoder::new(config).unwrap();
        assert_eq!(encoder.i420_buffer.len(), 1920 * 1080 * 3 / 2);
    }
}
