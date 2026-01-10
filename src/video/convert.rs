//! Pixel format conversion utilities
//!
//! This module provides SIMD-accelerated color space conversion using libyuv.
//! Primary use case: YUYV (from V4L2 capture) → YUV420P/NV12 (for H264 encoding)

use crate::error::{AppError, Result};
use crate::video::format::{PixelFormat, Resolution};

/// YUV420P buffer with separate Y, U, V planes
pub struct Yuv420pBuffer {
    /// Raw buffer containing all planes
    data: Vec<u8>,
    /// Width of the frame
    width: u32,
    /// Height of the frame
    height: u32,
    /// Y plane offset (always 0)
    y_offset: usize,
    /// U plane offset
    u_offset: usize,
    /// V plane offset
    v_offset: usize,
}

impl Yuv420pBuffer {
    /// Create a new YUV420P buffer for the given resolution
    pub fn new(resolution: Resolution) -> Self {
        let width = resolution.width;
        let height = resolution.height;

        // YUV420P: Y = width*height, U = width*height/4, V = width*height/4
        let y_size = (width * height) as usize;
        let uv_size = y_size / 4;
        let total_size = y_size + uv_size * 2;

        Self {
            data: vec![0u8; total_size],
            width,
            height,
            y_offset: 0,
            u_offset: y_size,
            v_offset: y_size + uv_size,
        }
    }

    /// Get the raw buffer as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get the raw buffer as mutable bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Get Y plane
    pub fn y_plane(&self) -> &[u8] {
        &self.data[self.y_offset..self.u_offset]
    }

    /// Get Y plane mutable
    pub fn y_plane_mut(&mut self) -> &mut [u8] {
        let u_offset = self.u_offset;
        &mut self.data[self.y_offset..u_offset]
    }

    /// Get U plane
    pub fn u_plane(&self) -> &[u8] {
        &self.data[self.u_offset..self.v_offset]
    }

    /// Get U plane mutable
    pub fn u_plane_mut(&mut self) -> &mut [u8] {
        let v_offset = self.v_offset;
        let u_offset = self.u_offset;
        &mut self.data[u_offset..v_offset]
    }

    /// Get V plane
    pub fn v_plane(&self) -> &[u8] {
        &self.data[self.v_offset..]
    }

    /// Get V plane mutable
    pub fn v_plane_mut(&mut self) -> &mut [u8] {
        let v_offset = self.v_offset;
        &mut self.data[v_offset..]
    }

    /// Get buffer length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get resolution
    pub fn resolution(&self) -> Resolution {
        Resolution::new(self.width, self.height)
    }
}

/// NV12 buffer with Y plane and interleaved UV plane
pub struct Nv12Buffer {
    /// Raw buffer containing Y plane followed by interleaved UV plane
    data: Vec<u8>,
    /// Width of the frame
    width: u32,
    /// Height of the frame
    height: u32,
}

impl Nv12Buffer {
    /// Create a new NV12 buffer for the given resolution
    pub fn new(resolution: Resolution) -> Self {
        let width = resolution.width;
        let height = resolution.height;
        // NV12: Y = width*height, UV = width*height/2 (interleaved)
        let y_size = (width * height) as usize;
        let uv_size = y_size / 2;
        let total_size = y_size + uv_size;

        Self {
            data: vec![0u8; total_size],
            width,
            height,
        }
    }

    /// Get the raw buffer as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get the raw buffer as mutable bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Get Y plane
    pub fn y_plane(&self) -> &[u8] {
        let y_size = (self.width * self.height) as usize;
        &self.data[..y_size]
    }

    /// Get Y plane mutable
    pub fn y_plane_mut(&mut self) -> &mut [u8] {
        let y_size = (self.width * self.height) as usize;
        &mut self.data[..y_size]
    }

    /// Get UV plane (interleaved)
    pub fn uv_plane(&self) -> &[u8] {
        let y_size = (self.width * self.height) as usize;
        &self.data[y_size..]
    }

    /// Get UV plane mutable
    pub fn uv_plane_mut(&mut self) -> &mut [u8] {
        let y_size = (self.width * self.height) as usize;
        &mut self.data[y_size..]
    }

    /// Get buffer length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get resolution
    pub fn resolution(&self) -> Resolution {
        Resolution::new(self.width, self.height)
    }
}

/// Pixel format converter using libyuv (SIMD accelerated)
pub struct PixelConverter {
    /// Source format
    src_format: PixelFormat,
    /// Destination format
    dst_format: PixelFormat,
    /// Frame resolution
    resolution: Resolution,
    /// Output buffer (reused across conversions)
    output_buffer: Yuv420pBuffer,
}

impl PixelConverter {
    /// Create a new converter for YUYV → YUV420P
    pub fn yuyv_to_yuv420p(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Yuyv,
            dst_format: PixelFormat::Yuv420,
            resolution,
            output_buffer: Yuv420pBuffer::new(resolution),
        }
    }

    /// Create a new converter for UYVY → YUV420P
    pub fn uyvy_to_yuv420p(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Uyvy,
            dst_format: PixelFormat::Yuv420,
            resolution,
            output_buffer: Yuv420pBuffer::new(resolution),
        }
    }

    /// Create a new converter for YVYU → YUV420P
    pub fn yvyu_to_yuv420p(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Yvyu,
            dst_format: PixelFormat::Yuv420,
            resolution,
            output_buffer: Yuv420pBuffer::new(resolution),
        }
    }

    /// Create a new converter for NV12 → YUV420P
    pub fn nv12_to_yuv420p(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Nv12,
            dst_format: PixelFormat::Yuv420,
            resolution,
            output_buffer: Yuv420pBuffer::new(resolution),
        }
    }

    /// Create a new converter for YVU420 → YUV420P (swap U and V planes)
    pub fn yvu420_to_yuv420p(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Yvu420,
            dst_format: PixelFormat::Yuv420,
            resolution,
            output_buffer: Yuv420pBuffer::new(resolution),
        }
    }

    /// Create a new converter for RGB24 → YUV420P
    pub fn rgb24_to_yuv420p(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Rgb24,
            dst_format: PixelFormat::Yuv420,
            resolution,
            output_buffer: Yuv420pBuffer::new(resolution),
        }
    }

    /// Create a new converter for BGR24 → YUV420P
    pub fn bgr24_to_yuv420p(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Bgr24,
            dst_format: PixelFormat::Yuv420,
            resolution,
            output_buffer: Yuv420pBuffer::new(resolution),
        }
    }

    /// Convert a frame and return reference to the output buffer
    pub fn convert(&mut self, input: &[u8]) -> Result<&[u8]> {
        let width = self.resolution.width as i32;
        let height = self.resolution.height as i32;
        let expected_size = self.output_buffer.len();

        match (self.src_format, self.dst_format) {
            (PixelFormat::Yuyv, PixelFormat::Yuv420) => {
                libyuv::yuy2_to_i420(input, self.output_buffer.as_bytes_mut(), width, height)
                    .map_err(|e| AppError::VideoError(format!("libyuv conversion failed: {}", e)))?;
            }
            (PixelFormat::Uyvy, PixelFormat::Yuv420) => {
                libyuv::uyvy_to_i420(input, self.output_buffer.as_bytes_mut(), width, height)
                    .map_err(|e| AppError::VideoError(format!("libyuv conversion failed: {}", e)))?;
            }
            (PixelFormat::Nv12, PixelFormat::Yuv420) => {
                libyuv::nv12_to_i420(input, self.output_buffer.as_bytes_mut(), width, height)
                    .map_err(|e| AppError::VideoError(format!("libyuv conversion failed: {}", e)))?;
            }
            (PixelFormat::Rgb24, PixelFormat::Yuv420) => {
                libyuv::rgb24_to_i420(input, self.output_buffer.as_bytes_mut(), width, height)
                    .map_err(|e| AppError::VideoError(format!("libyuv conversion failed: {}", e)))?;
            }
            (PixelFormat::Bgr24, PixelFormat::Yuv420) => {
                libyuv::bgr24_to_i420(input, self.output_buffer.as_bytes_mut(), width, height)
                    .map_err(|e| AppError::VideoError(format!("libyuv conversion failed: {}", e)))?;
            }
            (PixelFormat::Yvyu, PixelFormat::Yuv420) => {
                // YVYU is not directly supported by libyuv, use software conversion
                self.convert_yvyu_to_yuv420p_sw(input)?;
            }
            (PixelFormat::Yvu420, PixelFormat::Yuv420) => {
                // YVU420 just swaps U and V planes
                self.convert_yvu420_to_yuv420p_sw(input)?;
            }
            (PixelFormat::Yuv420, PixelFormat::Yuv420) => {
                // No conversion needed, just copy
                if input.len() < expected_size {
                    return Err(AppError::VideoError(format!(
                        "Input buffer too small: {} < {}",
                        input.len(),
                        expected_size
                    )));
                }
                self.output_buffer.as_bytes_mut().copy_from_slice(&input[..expected_size]);
            }
            _ => {
                return Err(AppError::VideoError(format!(
                    "Unsupported conversion: {} → {}",
                    self.src_format, self.dst_format
                )));
            }
        };

        Ok(self.output_buffer.as_bytes())
    }

    /// Get output buffer length
    pub fn output_len(&self) -> usize {
        self.output_buffer.len()
    }

    /// Get resolution
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Software conversion for YVYU (not supported by libyuv)
    fn convert_yvyu_to_yuv420p_sw(&mut self, yvyu: &[u8]) -> Result<()> {
        let width = self.resolution.width as usize;
        let height = self.resolution.height as usize;
        let y_size = width * height;
        let uv_size = y_size / 4;
        let half_width = width / 2;

        let data = self.output_buffer.as_bytes_mut();
        let (y_plane, uv_planes) = data.split_at_mut(y_size);
        let (u_plane, v_plane) = uv_planes.split_at_mut(uv_size);

        for row in (0..height).step_by(2) {
            let yvyu_row0_offset = row * width * 2;
            let yvyu_row1_offset = (row + 1) * width * 2;
            let y_row0_offset = row * width;
            let y_row1_offset = (row + 1) * width;
            let uv_row_offset = (row / 2) * half_width;

            for col in (0..width).step_by(2) {
                let yvyu_offset0 = yvyu_row0_offset + col * 2;
                let yvyu_offset1 = yvyu_row1_offset + col * 2;

                // YVYU: Y0, V0, Y1, U0
                let y0_0 = yvyu[yvyu_offset0];
                let v0 = yvyu[yvyu_offset0 + 1];
                let y0_1 = yvyu[yvyu_offset0 + 2];
                let u0 = yvyu[yvyu_offset0 + 3];

                let y1_0 = yvyu[yvyu_offset1];
                let v1 = yvyu[yvyu_offset1 + 1];
                let y1_1 = yvyu[yvyu_offset1 + 2];
                let u1 = yvyu[yvyu_offset1 + 3];

                y_plane[y_row0_offset + col] = y0_0;
                y_plane[y_row0_offset + col + 1] = y0_1;
                y_plane[y_row1_offset + col] = y1_0;
                y_plane[y_row1_offset + col + 1] = y1_1;

                let uv_idx = uv_row_offset + col / 2;
                u_plane[uv_idx] = ((u0 as u16 + u1 as u16) / 2) as u8;
                v_plane[uv_idx] = ((v0 as u16 + v1 as u16) / 2) as u8;
            }
        }
        Ok(())
    }

    /// Software conversion for YVU420 (just swap U and V)
    fn convert_yvu420_to_yuv420p_sw(&mut self, yvu420: &[u8]) -> Result<()> {
        let width = self.resolution.width as usize;
        let height = self.resolution.height as usize;
        let y_size = width * height;
        let uv_size = y_size / 4;

        let data = self.output_buffer.as_bytes_mut();
        let (y_plane, uv_planes) = data.split_at_mut(y_size);
        let (u_plane, v_plane) = uv_planes.split_at_mut(uv_size);

        // Copy Y plane directly
        y_plane.copy_from_slice(&yvu420[..y_size]);

        // In YVU420, V comes before U
        let v_src = &yvu420[y_size..y_size + uv_size];
        let u_src = &yvu420[y_size + uv_size..];

        // Swap U and V
        u_plane.copy_from_slice(u_src);
        v_plane.copy_from_slice(v_src);

        Ok(())
    }
}

/// Calculate YUV420P buffer size for a given resolution
pub fn yuv420p_buffer_size(resolution: Resolution) -> usize {
    let pixels = (resolution.width * resolution.height) as usize;
    pixels + pixels / 2
}

/// Calculate YUYV buffer size for a given resolution
pub fn yuyv_buffer_size(resolution: Resolution) -> usize {
    (resolution.width * resolution.height * 2) as usize
}

// ============================================================================
// NV12 Converter for VAAPI encoder (using libyuv)
// ============================================================================

/// Pixel format converter that outputs NV12 (for VAAPI encoders)
pub struct Nv12Converter {
    /// Source format
    src_format: PixelFormat,
    /// Frame resolution
    resolution: Resolution,
    /// Output buffer (reused across conversions)
    output_buffer: Nv12Buffer,
}

impl Nv12Converter {
    /// Create a new converter for BGR24 → NV12
    pub fn bgr24_to_nv12(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Bgr24,
            resolution,
            output_buffer: Nv12Buffer::new(resolution),
        }
    }

    /// Create a new converter for RGB24 → NV12
    pub fn rgb24_to_nv12(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Rgb24,
            resolution,
            output_buffer: Nv12Buffer::new(resolution),
        }
    }

    /// Create a new converter for YUYV → NV12
    pub fn yuyv_to_nv12(resolution: Resolution) -> Self {
        Self {
            src_format: PixelFormat::Yuyv,
            resolution,
            output_buffer: Nv12Buffer::new(resolution),
        }
    }

    /// Convert a frame and return reference to the output buffer
    pub fn convert(&mut self, input: &[u8]) -> Result<&[u8]> {
        let width = self.resolution.width as i32;
        let height = self.resolution.height as i32;
        let dst = self.output_buffer.as_bytes_mut();

        let result = match self.src_format {
            PixelFormat::Bgr24 => libyuv::bgr24_to_nv12(input, dst, width, height),
            PixelFormat::Rgb24 => libyuv::rgb24_to_nv12(input, dst, width, height),
            PixelFormat::Yuyv => libyuv::yuy2_to_nv12(input, dst, width, height),
            _ => {
                return Err(AppError::VideoError(format!(
                    "Unsupported conversion to NV12: {}",
                    self.src_format
                )));
            }
        };

        result.map_err(|e| AppError::VideoError(format!("libyuv NV12 conversion failed: {}", e)))?;
        Ok(self.output_buffer.as_bytes())
    }

    /// Get output buffer length
    pub fn output_len(&self) -> usize {
        self.output_buffer.len()
    }

    /// Get resolution
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }
}

// ============================================================================
// Standalone conversion functions (using libyuv)
// ============================================================================

/// Convert BGR24 to NV12 using libyuv
pub fn bgr_to_nv12(bgr: &[u8], nv12: &mut [u8], width: usize, height: usize) {
    if let Err(e) = libyuv::bgr24_to_nv12(bgr, nv12, width as i32, height as i32) {
        tracing::error!("libyuv BGR24→NV12 conversion failed: {}", e);
    }
}

/// Convert RGB24 to NV12 using libyuv
pub fn rgb_to_nv12(rgb: &[u8], nv12: &mut [u8], width: usize, height: usize) {
    if let Err(e) = libyuv::rgb24_to_nv12(rgb, nv12, width as i32, height as i32) {
        tracing::error!("libyuv RGB24→NV12 conversion failed: {}", e);
    }
}

/// Convert YUYV to NV12 using libyuv
pub fn yuyv_to_nv12(yuyv: &[u8], nv12: &mut [u8], width: usize, height: usize) {
    if let Err(e) = libyuv::yuy2_to_nv12(yuyv, nv12, width as i32, height as i32) {
        tracing::error!("libyuv YUYV→NV12 conversion failed: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yuv420p_buffer_creation() {
        let buffer = Yuv420pBuffer::new(Resolution::HD720);
        assert_eq!(buffer.len(), 1280 * 720 * 3 / 2);
        assert_eq!(buffer.y_plane().len(), 1280 * 720);
        assert_eq!(buffer.u_plane().len(), 1280 * 720 / 4);
        assert_eq!(buffer.v_plane().len(), 1280 * 720 / 4);
    }

    #[test]
    fn test_nv12_buffer_creation() {
        let buffer = Nv12Buffer::new(Resolution::HD720);
        assert_eq!(buffer.len(), 1280 * 720 * 3 / 2);
        assert_eq!(buffer.y_plane().len(), 1280 * 720);
        assert_eq!(buffer.uv_plane().len(), 1280 * 720 / 2);
    }

    #[test]
    fn test_yuyv_to_yuv420p_conversion() {
        let resolution = Resolution::new(4, 4);
        let mut converter = PixelConverter::yuyv_to_yuv420p(resolution);

        // Create YUYV data (4x4 = 32 bytes)
        let yuyv = vec![
            16, 128, 17, 129, 18, 130, 19, 131,
            20, 132, 21, 133, 22, 134, 23, 135,
            24, 136, 25, 137, 26, 138, 27, 139,
            28, 140, 29, 141, 30, 142, 31, 143,
        ];

        let result = converter.convert(&yuyv).unwrap();
        assert_eq!(result.len(), 24); // 4*4 + 2*2 + 2*2 = 24 bytes
    }
}
