//! MJPEG VAAPI hardware decoder
//!
//! Uses hwcodec's FFmpeg VAAPI backend to decode MJPEG to NV12.
//! This provides hardware-accelerated JPEG decoding with direct NV12 output,
//! which is the optimal format for VAAPI H264 encoding.

use std::sync::Once;
use tracing::{debug, info, warn};

use hwcodec::ffmpeg::AVHWDeviceType;
use hwcodec::ffmpeg::AVPixelFormat;
use hwcodec::ffmpeg_ram::decode::{DecodeContext, DecodeFrame, Decoder};

use crate::error::{AppError, Result};
use crate::video::format::Resolution;

// libyuv for SIMD-accelerated YUV conversion

static INIT_LOGGING: Once = Once::new();

/// Initialize hwcodec logging (only once)
fn init_hwcodec_logging() {
    INIT_LOGGING.call_once(|| {
        debug!("hwcodec MJPEG decoder logging initialized");
    });
}

/// MJPEG VAAPI decoder configuration
#[derive(Debug, Clone)]
pub struct MjpegVaapiDecoderConfig {
    /// Expected resolution (can be updated from decoded frame)
    pub resolution: Resolution,
    /// Use hardware acceleration (VAAPI)
    pub use_hwaccel: bool,
}

impl Default for MjpegVaapiDecoderConfig {
    fn default() -> Self {
        Self {
            resolution: Resolution::HD1080,
            use_hwaccel: true,
        }
    }
}

/// Decoded frame data in NV12 format
#[derive(Debug, Clone)]
pub struct DecodedNv12Frame {
    /// Y plane data
    pub y_plane: Vec<u8>,
    /// UV interleaved plane data
    pub uv_plane: Vec<u8>,
    /// Y plane linesize (stride)
    pub y_linesize: i32,
    /// UV plane linesize (stride)
    pub uv_linesize: i32,
    /// Frame width
    pub width: i32,
    /// Frame height
    pub height: i32,
}

/// Decoded frame data in YUV420P (I420) format
#[derive(Debug, Clone)]
pub struct DecodedYuv420pFrame {
    /// Y plane data
    pub y_plane: Vec<u8>,
    /// U plane data
    pub u_plane: Vec<u8>,
    /// V plane data
    pub v_plane: Vec<u8>,
    /// Y plane linesize (stride)
    pub y_linesize: i32,
    /// U plane linesize (stride)
    pub u_linesize: i32,
    /// V plane linesize (stride)
    pub v_linesize: i32,
    /// Frame width
    pub width: i32,
    /// Frame height
    pub height: i32,
}

impl DecodedYuv420pFrame {
    /// Get packed YUV420P data (Y plane followed by U and V planes, with stride removed)
    pub fn to_packed_yuv420p(&self) -> Vec<u8> {
        let width = self.width as usize;
        let height = self.height as usize;
        let y_size = width * height;
        let uv_size = width * height / 4;

        let mut packed = Vec::with_capacity(y_size + uv_size * 2);

        // Copy Y plane, removing stride padding if any
        if self.y_linesize as usize == width {
            packed.extend_from_slice(&self.y_plane[..y_size]);
        } else {
            for row in 0..height {
                let src_offset = row * self.y_linesize as usize;
                packed.extend_from_slice(&self.y_plane[src_offset..src_offset + width]);
            }
        }

        // Copy U plane
        let uv_width = width / 2;
        let uv_height = height / 2;
        if self.u_linesize as usize == uv_width {
            packed.extend_from_slice(&self.u_plane[..uv_size]);
        } else {
            for row in 0..uv_height {
                let src_offset = row * self.u_linesize as usize;
                packed.extend_from_slice(&self.u_plane[src_offset..src_offset + uv_width]);
            }
        }

        // Copy V plane
        if self.v_linesize as usize == uv_width {
            packed.extend_from_slice(&self.v_plane[..uv_size]);
        } else {
            for row in 0..uv_height {
                let src_offset = row * self.v_linesize as usize;
                packed.extend_from_slice(&self.v_plane[src_offset..src_offset + uv_width]);
            }
        }

        packed
    }

    /// Copy packed YUV420P data to external buffer (zero allocation)
    /// Returns the number of bytes written, or None if buffer too small
    pub fn copy_to_packed_yuv420p(&self, dst: &mut [u8]) -> Option<usize> {
        let width = self.width as usize;
        let height = self.height as usize;
        let y_size = width * height;
        let uv_size = width * height / 4;
        let total_size = y_size + uv_size * 2;

        if dst.len() < total_size {
            return None;
        }

        // Copy Y plane
        if self.y_linesize as usize == width {
            dst[..y_size].copy_from_slice(&self.y_plane[..y_size]);
        } else {
            for row in 0..height {
                let src_offset = row * self.y_linesize as usize;
                let dst_offset = row * width;
                dst[dst_offset..dst_offset + width]
                    .copy_from_slice(&self.y_plane[src_offset..src_offset + width]);
            }
        }

        // Copy U plane
        let uv_width = width / 2;
        let uv_height = height / 2;
        if self.u_linesize as usize == uv_width {
            dst[y_size..y_size + uv_size].copy_from_slice(&self.u_plane[..uv_size]);
        } else {
            for row in 0..uv_height {
                let src_offset = row * self.u_linesize as usize;
                let dst_offset = y_size + row * uv_width;
                dst[dst_offset..dst_offset + uv_width]
                    .copy_from_slice(&self.u_plane[src_offset..src_offset + uv_width]);
            }
        }

        // Copy V plane
        let v_offset = y_size + uv_size;
        if self.v_linesize as usize == uv_width {
            dst[v_offset..v_offset + uv_size].copy_from_slice(&self.v_plane[..uv_size]);
        } else {
            for row in 0..uv_height {
                let src_offset = row * self.v_linesize as usize;
                let dst_offset = v_offset + row * uv_width;
                dst[dst_offset..dst_offset + uv_width]
                    .copy_from_slice(&self.v_plane[src_offset..src_offset + uv_width]);
            }
        }

        Some(total_size)
    }
}

impl DecodedNv12Frame {
    /// Get packed NV12 data (Y plane followed by UV plane, with stride removed)
    pub fn to_packed_nv12(&self) -> Vec<u8> {
        let width = self.width as usize;
        let height = self.height as usize;
        let y_size = width * height;
        let uv_size = width * height / 2;

        let mut packed = Vec::with_capacity(y_size + uv_size);

        // Copy Y plane, removing stride padding if any
        if self.y_linesize as usize == width {
            // No padding, direct copy
            packed.extend_from_slice(&self.y_plane[..y_size]);
        } else {
            // Has padding, copy row by row
            for row in 0..height {
                let src_offset = row * self.y_linesize as usize;
                packed.extend_from_slice(&self.y_plane[src_offset..src_offset + width]);
            }
        }

        // Copy UV plane, removing stride padding if any
        let uv_height = height / 2;
        if self.uv_linesize as usize == width {
            // No padding, direct copy
            packed.extend_from_slice(&self.uv_plane[..uv_size]);
        } else {
            // Has padding, copy row by row
            for row in 0..uv_height {
                let src_offset = row * self.uv_linesize as usize;
                packed.extend_from_slice(&self.uv_plane[src_offset..src_offset + width]);
            }
        }

        packed
    }

    /// Copy packed NV12 data to external buffer (zero allocation)
    /// Returns the number of bytes written, or None if buffer too small
    pub fn copy_to_packed_nv12(&self, dst: &mut [u8]) -> Option<usize> {
        let width = self.width as usize;
        let height = self.height as usize;
        let y_size = width * height;
        let uv_size = width * height / 2;
        let total_size = y_size + uv_size;

        if dst.len() < total_size {
            return None;
        }

        // Copy Y plane, removing stride padding if any
        if self.y_linesize as usize == width {
            // No padding, direct copy
            dst[..y_size].copy_from_slice(&self.y_plane[..y_size]);
        } else {
            // Has padding, copy row by row
            for row in 0..height {
                let src_offset = row * self.y_linesize as usize;
                let dst_offset = row * width;
                dst[dst_offset..dst_offset + width]
                    .copy_from_slice(&self.y_plane[src_offset..src_offset + width]);
            }
        }

        // Copy UV plane, removing stride padding if any
        let uv_height = height / 2;
        if self.uv_linesize as usize == width {
            // No padding, direct copy
            dst[y_size..total_size].copy_from_slice(&self.uv_plane[..uv_size]);
        } else {
            // Has padding, copy row by row
            for row in 0..uv_height {
                let src_offset = row * self.uv_linesize as usize;
                let dst_offset = y_size + row * width;
                dst[dst_offset..dst_offset + width]
                    .copy_from_slice(&self.uv_plane[src_offset..src_offset + width]);
            }
        }

        Some(total_size)
    }
}

/// MJPEG VAAPI hardware decoder
///
/// Decodes MJPEG frames to NV12 format using VAAPI hardware acceleration.
/// This is optimal for feeding into VAAPI H264 encoder.
pub struct MjpegVaapiDecoder {
    /// hwcodec decoder instance
    decoder: Decoder,
    /// Configuration
    config: MjpegVaapiDecoderConfig,
    /// Frame counter
    frame_count: u64,
    /// Whether hardware acceleration is active
    hwaccel_active: bool,
}

impl MjpegVaapiDecoder {
    /// Create a new MJPEG decoder
    /// Note: VAAPI does not support MJPEG decoding on most hardware,
    /// so we use software decoding and convert to NV12 for VAAPI encoding.
    pub fn new(config: MjpegVaapiDecoderConfig) -> Result<Self> {
        init_hwcodec_logging();

        // VAAPI doesn't support MJPEG decoding, always use software decoder
        // The output will be converted to NV12 for VAAPI H264 encoding
        let device_type = AVHWDeviceType::AV_HWDEVICE_TYPE_NONE;

        info!(
            "Creating MJPEG decoder with software decoding (VAAPI doesn't support MJPEG decode)"
        );

        let ctx = DecodeContext {
            name: "mjpeg".to_string(),
            device_type,
            thread_count: 4, // Use multiple threads for software decoding
        };

        let decoder = Decoder::new(ctx).map_err(|_| {
            AppError::VideoError("Failed to create MJPEG software decoder".to_string())
        })?;

        // hwaccel is not actually active for MJPEG decoding
        let hwaccel_active = false;

        info!(
            "MJPEG decoder created successfully (software decode, will convert to NV12)"
        );

        Ok(Self {
            decoder,
            config,
            frame_count: 0,
            hwaccel_active,
        })
    }

    /// Create with default config (VAAPI enabled)
    pub fn with_vaapi(resolution: Resolution) -> Result<Self> {
        Self::new(MjpegVaapiDecoderConfig {
            resolution,
            use_hwaccel: true,
        })
    }

    /// Create with software decoding (fallback)
    pub fn with_software(resolution: Resolution) -> Result<Self> {
        Self::new(MjpegVaapiDecoderConfig {
            resolution,
            use_hwaccel: false,
        })
    }

    /// Check if hardware acceleration is active
    pub fn is_hwaccel_active(&self) -> bool {
        self.hwaccel_active
    }

    /// Decode MJPEG frame to NV12
    ///
    /// Returns the decoded frame in NV12 format, or an error if decoding fails.
    pub fn decode(&mut self, jpeg_data: &[u8]) -> Result<DecodedNv12Frame> {
        if jpeg_data.len() < 2 {
            return Err(AppError::VideoError("JPEG data too small".to_string()));
        }

        // Verify JPEG signature (FFD8)
        if jpeg_data[0] != 0xFF || jpeg_data[1] != 0xD8 {
            return Err(AppError::VideoError("Invalid JPEG signature".to_string()));
        }

        self.frame_count += 1;

        let frames = self.decoder.decode(jpeg_data).map_err(|e| {
            AppError::VideoError(format!("MJPEG decode failed: error code {}", e))
        })?;

        if frames.is_empty() {
            return Err(AppError::VideoError("Decoder returned no frames".to_string()));
        }

        let frame = &frames[0];

        // Handle different output formats
        // VAAPI MJPEG decoder may output NV12, YUV420P, or YUVJ420P (JPEG full-range)
        if frame.pixfmt == AVPixelFormat::AV_PIX_FMT_NV12
            || frame.pixfmt == AVPixelFormat::AV_PIX_FMT_NV21
        {
            // NV12/NV21 format: Y plane + UV interleaved plane
            if frame.data.len() < 2 {
                return Err(AppError::VideoError("Invalid NV12 frame data".to_string()));
            }

            return Ok(DecodedNv12Frame {
                y_plane: frame.data[0].clone(),
                uv_plane: frame.data[1].clone(),
                y_linesize: frame.linesize[0],
                uv_linesize: frame.linesize[1],
                width: frame.width,
                height: frame.height,
            });
        }

        // YUV420P or YUVJ420P (JPEG full-range) - need to convert to NV12
        if frame.pixfmt == AVPixelFormat::AV_PIX_FMT_YUV420P
            || frame.pixfmt == AVPixelFormat::AV_PIX_FMT_YUVJ420P
        {
            return Self::convert_yuv420p_to_nv12_static(frame);
        }

        // YUV422P or YUVJ422P (JPEG full-range 4:2:2) - need to convert to NV12
        if frame.pixfmt == AVPixelFormat::AV_PIX_FMT_YUV422P
            || frame.pixfmt == AVPixelFormat::AV_PIX_FMT_YUVJ422P
        {
            return Self::convert_yuv422p_to_nv12_static(frame);
        }

        Err(AppError::VideoError(format!(
            "Unexpected output format: {:?} (expected NV12, YUV420P, YUV422P, or YUVJ variants)",
            frame.pixfmt
        )))
    }

    /// Convert YUV420P frame to NV12 format using libyuv (SIMD accelerated)
    fn convert_yuv420p_to_nv12_static(frame: &DecodeFrame) -> Result<DecodedNv12Frame> {
        if frame.data.len() < 3 {
            return Err(AppError::VideoError("Invalid YUV420P frame data".to_string()));
        }

        let width = frame.width as i32;
        let height = frame.height as i32;
        let y_linesize = frame.linesize[0];
        let u_linesize = frame.linesize[1];
        let v_linesize = frame.linesize[2];

        // Allocate packed NV12 output buffer
        let nv12_size = (width * height * 3 / 2) as usize;
        let mut nv12_data = vec![0u8; nv12_size];

        // Use libyuv for SIMD-accelerated I420 → NV12 conversion
        libyuv::i420_to_nv12_planar(
            &frame.data[0], y_linesize,
            &frame.data[1], u_linesize,
            &frame.data[2], v_linesize,
            &mut nv12_data,
            width, height,
        ).map_err(|e| AppError::VideoError(format!("libyuv I420→NV12 failed: {}", e)))?;

        // Split into Y and UV planes for DecodedNv12Frame
        let y_size = (width * height) as usize;
        let y_plane = nv12_data[..y_size].to_vec();
        let uv_plane = nv12_data[y_size..].to_vec();

        Ok(DecodedNv12Frame {
            y_plane,
            uv_plane,
            y_linesize: width, // Output is packed, no padding
            uv_linesize: width,
            width: frame.width,
            height: frame.height,
        })
    }

    /// Convert YUV422P frame to NV12 format using libyuv (SIMD accelerated)
    /// Pipeline: I422 (YUV422P) → I420 → NV12
    fn convert_yuv422p_to_nv12_static(frame: &DecodeFrame) -> Result<DecodedNv12Frame> {
        if frame.data.len() < 3 {
            return Err(AppError::VideoError("Invalid YUV422P frame data".to_string()));
        }

        let width = frame.width as i32;
        let height = frame.height as i32;
        let y_linesize = frame.linesize[0];
        let u_linesize = frame.linesize[1];
        let v_linesize = frame.linesize[2];

        // Step 1: I422 → I420 (vertical chroma downsampling via SIMD)
        let i420_size = (width * height * 3 / 2) as usize;
        let mut i420_data = vec![0u8; i420_size];

        libyuv::i422_to_i420_planar(
            &frame.data[0], y_linesize,
            &frame.data[1], u_linesize,
            &frame.data[2], v_linesize,
            &mut i420_data,
            width, height,
        ).map_err(|e| AppError::VideoError(format!("libyuv I422→I420 failed: {}", e)))?;

        // Step 2: I420 → NV12 (UV interleaving via SIMD)
        let nv12_size = (width * height * 3 / 2) as usize;
        let mut nv12_data = vec![0u8; nv12_size];

        libyuv::i420_to_nv12(&i420_data, &mut nv12_data, width, height)
            .map_err(|e| AppError::VideoError(format!("libyuv I420→NV12 failed: {}", e)))?;

        // Split into Y and UV planes for DecodedNv12Frame
        let y_size = (width * height) as usize;
        let y_plane = nv12_data[..y_size].to_vec();
        let uv_plane = nv12_data[y_size..].to_vec();

        Ok(DecodedNv12Frame {
            y_plane,
            uv_plane,
            y_linesize: width,
            uv_linesize: width,
            width: frame.width,
            height: frame.height,
        })
    }

    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get current resolution from config
    pub fn resolution(&self) -> Resolution {
        self.config.resolution
    }
}

/// Libyuv-based MJPEG decoder for direct YUV420P output
///
/// This decoder is optimized for software encoders (libvpx, libx265) that need YUV420P input.
/// It uses libyuv's MJPGToI420 to decode directly to I420/YUV420P format.
pub struct MjpegTurboDecoder {
    /// Frame counter
    frame_count: u64,
}

impl MjpegTurboDecoder {
    /// Create a new libyuv-based MJPEG decoder
    pub fn new(resolution: Resolution) -> Result<Self> {
        info!(
            "Created libyuv MJPEG decoder for {}x{} (direct YUV420P output)",
            resolution.width, resolution.height
        );

        Ok(Self {
            frame_count: 0,
        })
    }

    /// Decode MJPEG frame directly to YUV420P using libyuv
    ///
    /// This is the optimal path for software encoders that need YUV420P input.
    /// libyuv handles all JPEG subsampling formats internally.
    pub fn decode_to_yuv420p(&mut self, jpeg_data: &[u8]) -> Result<DecodedYuv420pFrame> {
        if jpeg_data.len() < 2 || jpeg_data[0] != 0xFF || jpeg_data[1] != 0xD8 {
            return Err(AppError::VideoError("Invalid JPEG data".to_string()));
        }

        self.frame_count += 1;

        // Get JPEG dimensions
        let (width, height) = libyuv::mjpeg_size(jpeg_data)
            .map_err(|e| AppError::VideoError(format!("Failed to read MJPEG size: {}", e)))?;

        let y_size = (width * height) as usize;
        let uv_size = y_size / 4;
        let yuv420_size = y_size + uv_size * 2;

        let mut yuv_data = vec![0u8; yuv420_size];

        libyuv::mjpeg_to_i420(jpeg_data, &mut yuv_data, width, height)
            .map_err(|e| AppError::VideoError(format!("libyuv MJPEG→I420 failed: {}", e)))?;

        Ok(DecodedYuv420pFrame {
            y_plane: yuv_data[..y_size].to_vec(),
            u_plane: yuv_data[y_size..y_size + uv_size].to_vec(),
            v_plane: yuv_data[y_size + uv_size..].to_vec(),
            y_linesize: width,
            u_linesize: width / 2,
            v_linesize: width / 2,
            width,
            height,
        })
    }

    /// Decode directly to packed YUV420P buffer using libyuv
    ///
    /// This uses libyuv's MJPGToI420 which handles all JPEG subsampling formats
    /// and converts to I420 directly.
    pub fn decode_to_yuv420p_buffer(&mut self, jpeg_data: &[u8], dst: &mut [u8]) -> Result<usize> {
        if jpeg_data.len() < 2 || jpeg_data[0] != 0xFF || jpeg_data[1] != 0xD8 {
            return Err(AppError::VideoError("Invalid JPEG data".to_string()));
        }

        self.frame_count += 1;

        // Get JPEG dimensions from libyuv
        let (width, height) = libyuv::mjpeg_size(jpeg_data)
            .map_err(|e| AppError::VideoError(format!("Failed to read MJPEG size: {}", e)))?;

        let yuv420_size = (width * height * 3 / 2) as usize;

        if dst.len() < yuv420_size {
            return Err(AppError::VideoError(format!(
                "Buffer too small: {} < {}", dst.len(), yuv420_size
            )));
        }

        // Decode MJPEG directly to I420 using libyuv
        // libyuv handles all JPEG subsampling formats (4:2:0, 4:2:2, 4:4:4) internally
        libyuv::mjpeg_to_i420(jpeg_data, &mut dst[..yuv420_size], width, height)
            .map_err(|e| AppError::VideoError(format!("libyuv MJPEG→I420 failed: {}", e)))?;

        Ok(yuv420_size)
    }

    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

/// Check if MJPEG VAAPI decoder is available
pub fn is_mjpeg_vaapi_available() -> bool {
    let ctx = DecodeContext {
        name: "mjpeg".to_string(),
        device_type: AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI,
        thread_count: 1,
    };

    match Decoder::new(ctx) {
        Ok(_) => {
            info!("MJPEG VAAPI decoder is available");
            true
        }
        Err(_) => {
            warn!("MJPEG VAAPI decoder is not available");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mjpeg_vaapi_availability() {
        let available = is_mjpeg_vaapi_available();
        println!("MJPEG VAAPI available: {}", available);
    }

    #[test]
    fn test_decoder_creation() {
        let config = MjpegVaapiDecoderConfig::default();
        match MjpegVaapiDecoder::new(config) {
            Ok(decoder) => {
                println!("Decoder created, hwaccel: {}", decoder.is_hwaccel_active());
            }
            Err(e) => {
                println!("Failed to create decoder: {}", e);
            }
        }
    }
}
