//! MJPEG decoder implementations
//!
//! Provides MJPEG decoding using libyuv for SIMD-accelerated decoding.
//! All decoders output to standard YUV formats suitable for encoding.

use std::sync::Once;
use tracing::{debug, info};

use crate::error::{AppError, Result};
use crate::video::format::Resolution;

static INIT_LOGGING: Once = Once::new();

/// Initialize decoder logging (only once)
fn init_decoder_logging() {
    INIT_LOGGING.call_once(|| {
        debug!("MJPEG decoder logging initialized");
    });
}

/// MJPEG decoder configuration
#[derive(Debug, Clone)]
pub struct MjpegVaapiDecoderConfig {
    /// Expected resolution (can be updated from decoded frame)
    pub resolution: Resolution,
    /// Use hardware acceleration (ignored, kept for API compatibility)
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

/// MJPEG decoder with NV12 output
///
/// Uses libyuv for SIMD-accelerated MJPEG decoding to YUV420P,
/// then converts to NV12 for hardware encoder compatibility.
/// Named "VaapiDecoder" for API compatibility with existing code.
pub struct MjpegVaapiDecoder {
    /// Configuration
    config: MjpegVaapiDecoderConfig,
    /// Frame counter
    frame_count: u64,
}

impl MjpegVaapiDecoder {
    /// Create a new MJPEG decoder
    pub fn new(config: MjpegVaapiDecoderConfig) -> Result<Self> {
        init_decoder_logging();

        info!(
            "Creating MJPEG decoder with libyuv (SIMD-accelerated, NV12 output)"
        );

        Ok(Self {
            config,
            frame_count: 0,
        })
    }

    /// Create with default config
    pub fn with_vaapi(resolution: Resolution) -> Result<Self> {
        Self::new(MjpegVaapiDecoderConfig {
            resolution,
            use_hwaccel: true,
        })
    }

    /// Create with software decoding (same as with_vaapi, kept for API compatibility)
    pub fn with_software(resolution: Resolution) -> Result<Self> {
        Self::new(MjpegVaapiDecoderConfig {
            resolution,
            use_hwaccel: false,
        })
    }

    /// Check if hardware acceleration is active (always false, using libyuv)
    pub fn is_hwaccel_active(&self) -> bool {
        false
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

        // Get JPEG dimensions
        let (width, height) = libyuv::mjpeg_size(jpeg_data)
            .map_err(|e| AppError::VideoError(format!("Failed to read MJPEG size: {}", e)))?;

        // Decode MJPEG to YUV420P first
        let y_size = (width * height) as usize;
        let uv_size = y_size / 4;
        let yuv420_size = y_size + uv_size * 2;
        let mut yuv_data = vec![0u8; yuv420_size];

        libyuv::mjpeg_to_i420(jpeg_data, &mut yuv_data, width, height)
            .map_err(|e| AppError::VideoError(format!("libyuv MJPEG→I420 failed: {}", e)))?;

        // Convert I420 to NV12
        let nv12_size = (width * height * 3 / 2) as usize;
        let mut nv12_data = vec![0u8; nv12_size];

        libyuv::i420_to_nv12(&yuv_data, &mut nv12_data, width, height)
            .map_err(|e| AppError::VideoError(format!("libyuv I420→NV12 failed: {}", e)))?;

        // Split into Y and UV planes
        let y_plane = nv12_data[..y_size].to_vec();
        let uv_plane = nv12_data[y_size..].to_vec();

        Ok(DecodedNv12Frame {
            y_plane,
            uv_plane,
            y_linesize: width,
            uv_linesize: width,
            width,
            height,
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

#[cfg(test)]
mod tests {
    use super::*;

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
