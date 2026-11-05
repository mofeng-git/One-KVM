//! JPEG encoder implementation
//!
//! Provides JPEG encoding for raw video frames (YUYV, NV12, NV16, NV24, RGB, BGR)
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
/// YUYV/NV12/NV16/NV24/BGR24/RGB24 ──libyuv──> I420 ──turbojpeg──> JPEG
/// ```
///
/// Note: This encoder is NOT thread-safe due to turbojpeg limitations.
/// Use it from a single thread or wrap in a Mutex.
pub struct JpegEncoder {
    config: EncoderConfig,
    compressor: turbojpeg::Compressor,
    /// I420 buffer for YUV encoding (Y + U + V planes)
    i420_buffer: Vec<u8>,
    /// Scratch buffer for split chroma planes when converting semiplanar 4:2:2 / 4:4:4 input.
    uv_split_buffer: Vec<u8>,
    /// BGRA buffer used when a source format needs explicit YUV matrix expansion before JPEG.
    bgra_buffer: Vec<u8>,
}

impl JpegEncoder {
    /// Create a new JPEG encoder
    pub fn new(config: EncoderConfig) -> Result<Self> {
        let mut compressor = turbojpeg::Compressor::new().map_err(|e| {
            AppError::VideoError(format!("Failed to create turbojpeg compressor: {}", e))
        })?;

        compressor
            .set_quality(config.quality.min(100) as i32)
            .map_err(|e| AppError::VideoError(format!("Failed to set JPEG quality: {}", e)))?;

        Ok(Self {
            config,
            compressor,
            i420_buffer: Vec::new(),
            uv_split_buffer: Vec::new(),
            bgra_buffer: Vec::new(),
        })
    }

    fn ensure_i420_buffer(&mut self) {
        let size =
            self.config.resolution.width as usize * self.config.resolution.height as usize * 3 / 2;
        self.i420_buffer.resize(size, 0);
    }

    /// Create with specific quality
    pub fn with_quality(resolution: Resolution, quality: u32) -> Result<Self> {
        let config = EncoderConfig::jpeg(resolution, quality);
        Self::new(config)
    }

    /// Set JPEG quality (1-100)
    pub fn set_quality(&mut self, quality: u32) -> Result<()> {
        self.compressor
            .set_quality(quality.min(100) as i32)
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
            align: 1,                            // No padding between rows
            subsamp: turbojpeg::Subsamp::Sub2x2, // YUV 4:2:0
        };

        // Compress YUV directly to JPEG (skips color space conversion!)
        let jpeg_data = self
            .compressor
            .compress_yuv_to_vec(yuv_image)
            .map_err(|e| AppError::VideoError(format!("JPEG compression failed: {}", e)))?;

        Ok(EncodedFrame::jpeg(
            Bytes::from(jpeg_data),
            self.config.resolution,
            sequence,
        ))
    }

    /// Encode BGRA buffer to JPEG using turbojpeg's RGB path.
    #[inline]
    fn encode_bgra_to_jpeg(&mut self, sequence: u64) -> Result<EncodedFrame> {
        let width = self.config.resolution.width as usize;
        let height = self.config.resolution.height as usize;

        self.compressor
            .set_subsamp(turbojpeg::Subsamp::Sub2x2)
            .map_err(|e| AppError::VideoError(format!("Failed to set JPEG subsampling: {}", e)))?;

        let image = turbojpeg::Image {
            pixels: self.bgra_buffer.as_slice(),
            width,
            pitch: width * 4,
            height,
            format: turbojpeg::PixelFormat::BGRA,
        };

        let jpeg_data = self
            .compressor
            .compress_to_vec(image)
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
        self.ensure_i420_buffer();
        libyuv::yuy2_to_i420(data, &mut self.i420_buffer, width as i32, height as i32)
            .map_err(|e| AppError::VideoError(format!("libyuv YUYV→I420 failed: {}", e)))?;

        self.encode_i420_to_jpeg(sequence)
    }

    /// YVYU → swap chroma to YUYV in scratch, then same as [`Self::encode_yuyv`].
    pub fn encode_yvyu(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        let width = self.config.resolution.width as usize;
        let height = self.config.resolution.height as usize;
        let expected_size = width * height * 2;

        if data.len() < expected_size {
            return Err(AppError::VideoError(format!(
                "YVYU data too small: {} < {}",
                data.len(),
                expected_size
            )));
        }

        self.ensure_i420_buffer();
        // Reuse bgra_buffer as scratch for the swapped YUYV data.
        if self.bgra_buffer.len() < expected_size {
            self.bgra_buffer.resize(expected_size, 0);
        }
        let dst = &mut self.bgra_buffer[..expected_size];
        let src = &data[..expected_size];

        // Swap bytes [1] and [3] in every 4-byte macropixel: Y0 V0 Y1 U0 → Y0 U0 Y1 V0
        for (chunk_dst, chunk_src) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            chunk_dst[0] = chunk_src[0]; // Y0
            chunk_dst[1] = chunk_src[3]; // U0
            chunk_dst[2] = chunk_src[2]; // Y1
            chunk_dst[3] = chunk_src[1]; // V0
        }

        libyuv::yuy2_to_i420(dst, &mut self.i420_buffer, width as i32, height as i32)
            .map_err(|e| AppError::VideoError(format!("libyuv YVYU→I420 failed: {}", e)))?;

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
        self.ensure_i420_buffer();
        libyuv::nv12_to_i420(data, &mut self.i420_buffer, width as i32, height as i32)
            .map_err(|e| AppError::VideoError(format!("libyuv NV12→I420 failed: {}", e)))?;

        self.encode_i420_to_jpeg(sequence)
    }

    /// Encode NV16 frame to JPEG
    pub fn encode_nv16(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        let width = self.config.resolution.width as usize;
        let height = self.config.resolution.height as usize;
        let y_size = width * height;
        let uv_size = y_size;
        let expected_size = y_size + uv_size;

        if data.len() < expected_size {
            return Err(AppError::VideoError(format!(
                "NV16 data too small: {} < {}",
                data.len(),
                expected_size
            )));
        }

        self.ensure_i420_buffer();
        self.uv_split_buffer.resize(y_size, 0);
        let src_uv = &data[y_size..expected_size];
        let chroma_plane_size = y_size / 2;
        let (u_plane_422, rest) = self.uv_split_buffer.split_at_mut(chroma_plane_size);
        let (v_plane_422, _) = rest.split_at_mut(chroma_plane_size);

        libyuv::split_uv_plane(
            src_uv,
            width as i32,
            u_plane_422,
            (width / 2) as i32,
            v_plane_422,
            (width / 2) as i32,
            (width / 2) as i32,
            height as i32,
        )
        .map_err(|e| AppError::VideoError(format!("libyuv NV16 split failed: {}", e)))?;

        libyuv::i422_to_i420_planar(
            &data[..y_size],
            width as i32,
            u_plane_422,
            (width / 2) as i32,
            v_plane_422,
            (width / 2) as i32,
            &mut self.i420_buffer,
            width as i32,
            height as i32,
        )
        .map_err(|e| AppError::VideoError(format!("libyuv NV16→I420 failed: {}", e)))?;

        self.encode_i420_to_jpeg(sequence)
    }

    /// Encode NV24 frame to JPEG
    pub fn encode_nv24(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        let width = self.config.resolution.width as usize;
        let height = self.config.resolution.height as usize;
        let y_size = width * height;
        let uv_size = y_size * 2;
        let expected_size = y_size + uv_size;

        if data.len() < expected_size {
            return Err(AppError::VideoError(format!(
                "NV24 data too small: {} < {}",
                data.len(),
                expected_size
            )));
        }

        self.uv_split_buffer.resize(y_size * 2, 0);
        self.bgra_buffer.resize(y_size * 4, 0);
        let src_uv = &data[y_size..expected_size];
        let chroma_plane_size = y_size;
        let (u_plane_444, rest) = self.uv_split_buffer.split_at_mut(chroma_plane_size);
        let (v_plane_444, _) = rest.split_at_mut(chroma_plane_size);

        libyuv::split_uv_plane(
            src_uv,
            (width * 2) as i32,
            u_plane_444,
            width as i32,
            v_plane_444,
            width as i32,
            width as i32,
            height as i32,
        )
        .map_err(|e| AppError::VideoError(format!("libyuv NV24 split failed: {}", e)))?;

        libyuv::h444_to_bgra(
            &data[..y_size],
            u_plane_444,
            v_plane_444,
            &mut self.bgra_buffer,
            width as i32,
            height as i32,
        )
        .map_err(|e| AppError::VideoError(format!("libyuv NV24(H444)→BGRA failed: {}", e)))?;

        self.encode_bgra_to_jpeg(sequence)
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
        self.ensure_i420_buffer();
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
        self.ensure_i420_buffer();
        libyuv::bgr24_to_i420(data, &mut self.i420_buffer, width as i32, height as i32)
            .map_err(|e| AppError::VideoError(format!("libyuv BGR24→I420 failed: {}", e)))?;

        self.encode_i420_to_jpeg(sequence)
    }
}

impl crate::video::codec::traits::Encoder for JpegEncoder {
    fn name(&self) -> &str {
        "JPEG (libyuv+turbojpeg)"
    }

    fn output_format(&self) -> EncodedFormat {
        EncodedFormat::Jpeg
    }

    fn encode(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        match self.config.input_format {
            PixelFormat::Yuyv => self.encode_yuyv(data, sequence),
            PixelFormat::Yvyu => self.encode_yvyu(data, sequence),
            PixelFormat::Nv12 => self.encode_nv12(data, sequence),
            PixelFormat::Nv16 => self.encode_nv16(data, sequence),
            PixelFormat::Nv24 => self.encode_nv24(data, sequence),
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
                | PixelFormat::Nv16
                | PixelFormat::Nv24
                | PixelFormat::Rgb24
                | PixelFormat::Bgr24
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_buffers_are_allocated_only_for_the_formats_that_use_them() {
        let resolution = Resolution::new(64, 32);
        let pixels = 64 * 32;
        let mut encoder = JpegEncoder::new(EncoderConfig::jpeg(resolution, 80)).unwrap();
        assert!(encoder.i420_buffer.is_empty());
        assert!(encoder.uv_split_buffer.is_empty());
        assert!(encoder.bgra_buffer.is_empty());
        encoder.encode_yuyv(&vec![128; pixels * 2], 0).unwrap();
        encoder.encode_nv12(&vec![128; pixels * 3 / 2], 1).unwrap();
        encoder.encode_rgb(&vec![128; pixels * 3], 2).unwrap();
        encoder.encode_bgr(&vec![128; pixels * 3], 3).unwrap();
        assert_eq!(encoder.i420_buffer.len(), pixels * 3 / 2);
        assert!(encoder.uv_split_buffer.is_empty());
        assert!(encoder.bgra_buffer.is_empty());

        encoder.encode_yvyu(&vec![128; pixels * 2], 4).unwrap();
        assert_eq!(encoder.bgra_buffer.len(), pixels * 2);
        encoder.encode_nv16(&vec![128; pixels * 2], 5).unwrap();
        assert_eq!(encoder.uv_split_buffer.len(), pixels);
        encoder.encode_nv24(&vec![128; pixels * 3], 6).unwrap();
        assert_eq!(encoder.uv_split_buffer.len(), pixels * 2);
        assert_eq!(encoder.bgra_buffer.len(), pixels * 4);

        let mut nv24 = JpegEncoder::new(EncoderConfig::jpeg(resolution, 80)).unwrap();
        nv24.encode_nv24(&vec![128; pixels * 3], 0).unwrap();
        assert!(nv24.i420_buffer.is_empty());
    }
}
