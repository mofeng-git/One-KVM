//! libyuv bindings for high-performance pixel format conversion
//!
//! This crate provides safe Rust bindings to Google's libyuv library,
//! which offers SIMD-accelerated (SSE/AVX/NEON) color space conversion.
//!
//! # Features
//!
//! - Zero-copy conversion (operates directly on slices)
//! - SIMD acceleration on x86_64 (SSE2/AVX2) and ARM (NEON)
//! - Support for common video formats: YUYV, UYVY, NV12, I420, RGB, MJPEG
//! - Scaling and rotation support
//!
//! # Example
//!
//! ```ignore
//! use libyuv::{yuy2_to_nv12, nv12_size};
//!
//! let width = 1920;
//! let height = 1080;
//! let yuyv_data: &[u8] = &[/* YUYV data from capture */];
//! let mut nv12_data = vec![0u8; nv12_size(width, height)];
//!
//! yuy2_to_nv12(yuyv_data, &mut nv12_data, width as i32, height as i32).unwrap();
//! ```

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use std::fmt;
// Include auto-generated FFI bindings
include!(concat!(env!("OUT_DIR"), "/yuv_ffi.rs"));

// Type alias for C's size_t - adapts to platform pointer width
#[cfg(target_pointer_width = "32")]
type SizeT = u32;

#[cfg(target_pointer_width = "64")]
type SizeT = u64;

// Helper function to convert usize to C's size_t type
#[inline]
fn usize_to_size_t(val: usize) -> SizeT {
    val as SizeT
}

// ============================================================================
// Error types
// ============================================================================

/// Error type for libyuv operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YuvError {
    /// Dimensions must be even numbers
    InvalidDimensions,
    /// Input or output buffer is too small
    BufferTooSmall,
    /// libyuv function returned an error code
    ConversionFailed(i32),
    /// MJPEG data is invalid or corrupt
    InvalidMjpeg,
}

impl fmt::Display for YuvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            YuvError::InvalidDimensions => write!(f, "Invalid dimensions (must be even)"),
            YuvError::BufferTooSmall => write!(f, "Buffer too small"),
            YuvError::ConversionFailed(code) => write!(f, "Conversion failed with code {}", code),
            YuvError::InvalidMjpeg => write!(f, "Invalid MJPEG data"),
        }
    }
}

impl std::error::Error for YuvError {}

pub type Result<T> = std::result::Result<T, YuvError>;

/// Macro to call libyuv functions and check return value
macro_rules! call_yuv {
    ($func:expr) => {{
        let ret = unsafe { $func };
        if ret != 0 {
            return Err(YuvError::ConversionFailed(ret));
        }
        Ok(())
    }};
}

// ============================================================================
// Buffer size calculations
// ============================================================================

/// Calculate I420 (YUV420P) buffer size
#[inline]
pub const fn i420_size(width: usize, height: usize) -> usize {
    width * height + (width / 2) * (height / 2) * 2
}

/// Calculate NV12 buffer size
#[inline]
pub const fn nv12_size(width: usize, height: usize) -> usize {
    width * height * 3 / 2
}

/// Calculate YUYV/UYVY buffer size
#[inline]
pub const fn yuyv_size(width: usize, height: usize) -> usize {
    width * height * 2
}

/// Calculate RGB24/BGR24 buffer size
#[inline]
pub const fn rgb24_size(width: usize, height: usize) -> usize {
    width * height * 3
}

/// Calculate ARGB/BGRA buffer size
#[inline]
pub const fn argb_size(width: usize, height: usize) -> usize {
    width * height * 4
}

// ============================================================================
// YUYV (YUY2) conversions
// ============================================================================

/// Convert YUYV (YUY2) to I420 (YUV420P)
///
/// # Arguments
/// * `src` - Source YUYV data
/// * `dst` - Destination I420 buffer
/// * `width` - Frame width (must be even)
/// * `height` - Frame height (must be even)
pub fn yuy2_to_i420(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < yuyv_size(w, h) || dst.len() < i420_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(YUY2ToI420(
        src.as_ptr(),
        width * 2,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width / 2,
        dst[y_size + uv_size..].as_mut_ptr(),
        width / 2,
        width,
        height,
    ))
}

/// Convert YUYV (YUY2) to NV12 (optimal for VAAPI)
///
/// # Arguments
/// * `src` - Source YUYV data
/// * `dst` - Destination NV12 buffer
/// * `width` - Frame width (must be even)
/// * `height` - Frame height (must be even)
pub fn yuy2_to_nv12(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;

    if src.len() < yuyv_size(w, h) || dst.len() < nv12_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(YUY2ToNV12(
        src.as_ptr(),
        width * 2,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width,
        width,
        height,
    ))
}

// ============================================================================
// UYVY conversions
// ============================================================================

/// Convert UYVY to I420
pub fn uyvy_to_i420(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < yuyv_size(w, h) || dst.len() < i420_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(UYVYToI420(
        src.as_ptr(),
        width * 2,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width / 2,
        dst[y_size + uv_size..].as_mut_ptr(),
        width / 2,
        width,
        height,
    ))
}

/// Convert UYVY to NV12
pub fn uyvy_to_nv12(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;

    if src.len() < yuyv_size(w, h) || dst.len() < nv12_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(UYVYToNV12(
        src.as_ptr(),
        width * 2,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width,
        width,
        height,
    ))
}

// ============================================================================
// I422 (YUV422P) -> I420 conversion
// ============================================================================

/// Convert I422 (YUV422P) to I420 (YUV420P) with separate planes and explicit strides
/// This performs vertical 2:1 chroma downsampling using SIMD
pub fn i422_to_i420_planar(
    src_y: &[u8],
    src_y_stride: i32,
    src_u: &[u8],
    src_u_stride: i32,
    src_v: &[u8],
    src_v_stride: i32,
    dst: &mut [u8],
    width: i32,
    height: i32,
) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if dst.len() < i420_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(I422ToI420(
        src_y.as_ptr(),
        src_y_stride,
        src_u.as_ptr(),
        src_u_stride,
        src_v.as_ptr(),
        src_v_stride,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width / 2,
        dst[y_size + uv_size..].as_mut_ptr(),
        width / 2,
        width,
        height,
    ))
}

// ============================================================================
// I420 <-> NV12 conversions
// ============================================================================

/// Convert I420 (YUV420P) to NV12
pub fn i420_to_nv12(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < i420_size(w, h) || dst.len() < nv12_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(I420ToNV12(
        src.as_ptr(),
        width,
        src[y_size..].as_ptr(),
        width / 2,
        src[y_size + uv_size..].as_ptr(),
        width / 2,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width,
        width,
        height,
    ))
}

/// Convert I420 (YUV420P) to NV12 with separate planes and explicit strides
/// This is useful when working with decoder output that has stride padding
pub fn i420_to_nv12_planar(
    y_plane: &[u8],
    y_stride: i32,
    u_plane: &[u8],
    u_stride: i32,
    v_plane: &[u8],
    v_stride: i32,
    dst: &mut [u8],
    width: i32,
    height: i32,
) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;

    if dst.len() < nv12_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(I420ToNV12(
        y_plane.as_ptr(),
        y_stride,
        u_plane.as_ptr(),
        u_stride,
        v_plane.as_ptr(),
        v_stride,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width,
        width,
        height,
    ))
}

/// Convert NV12 to I420 (YUV420P)
pub fn nv12_to_i420(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < nv12_size(w, h) || dst.len() < i420_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(NV12ToI420(
        src.as_ptr(),
        width,
        src[y_size..].as_ptr(),
        width,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width / 2,
        dst[y_size + uv_size..].as_mut_ptr(),
        width / 2,
        width,
        height,
    ))
}

// ============================================================================
// ARGB/BGRA conversions (32-bit)
// Note: libyuv ARGB = BGRA in memory on little-endian systems
// ============================================================================

/// Convert BGRA to I420
///
/// Note: In libyuv, ARGB means BGRA byte order in memory
pub fn bgra_to_i420(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < argb_size(w, h) || dst.len() < i420_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(ARGBToI420(
        src.as_ptr(),
        width * 4,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width / 2,
        dst[y_size + uv_size..].as_mut_ptr(),
        width / 2,
        width,
        height,
    ))
}

/// Convert BGRA to NV12
pub fn bgra_to_nv12(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;

    if src.len() < argb_size(w, h) || dst.len() < nv12_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(ARGBToNV12(
        src.as_ptr(),
        width * 4,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width,
        width,
        height,
    ))
}

/// Convert RGBA to I420
///
/// Note: In libyuv, ABGR means RGBA byte order in memory
pub fn rgba_to_i420(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < argb_size(w, h) || dst.len() < i420_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(ABGRToI420(
        src.as_ptr(),
        width * 4,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width / 2,
        dst[y_size + uv_size..].as_mut_ptr(),
        width / 2,
        width,
        height,
    ))
}

/// Convert RGBA to NV12
pub fn rgba_to_nv12(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;

    if src.len() < argb_size(w, h) || dst.len() < nv12_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(ABGRToNV12(
        src.as_ptr(),
        width * 4,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width,
        width,
        height,
    ))
}

/// Convert BGRA to RGBA (swap R and B channels)
pub fn bgra_to_rgba(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;
    let size = argb_size(w, h);

    if src.len() < size || dst.len() < size {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(ARGBToABGR(
        src.as_ptr(),
        width * 4,
        dst.as_mut_ptr(),
        width * 4,
        width,
        height,
    ))
}

/// Convert RGBA to BGRA (swap R and B channels)
pub fn rgba_to_bgra(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;
    let size = argb_size(w, h);

    if src.len() < size || dst.len() < size {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(ABGRToARGB(
        src.as_ptr(),
        width * 4,
        dst.as_mut_ptr(),
        width * 4,
        width,
        height,
    ))
}

// ============================================================================
// RGB24/BGR24 conversions (24-bit)
// Note: libyuv naming is confusing - RGB24 in libyuv is actually BGR in memory!
// ============================================================================

/// Convert RGB24 to I420
/// Note: Uses RAWToI420 because libyuv's "RAW" is actually RGB order
pub fn rgb24_to_i420(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < rgb24_size(w, h) || dst.len() < i420_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    // libyuv RAW = RGB byte order in memory
    call_yuv!(RAWToI420(
        src.as_ptr(),
        width * 3,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width / 2,
        dst[y_size + uv_size..].as_mut_ptr(),
        width / 2,
        width,
        height,
    ))
}

/// Convert BGR24 to I420
/// Note: Uses RGB24ToI420 because libyuv's "RGB24" is actually BGR order
pub fn bgr24_to_i420(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < rgb24_size(w, h) || dst.len() < i420_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    // libyuv RGB24 = BGR byte order in memory
    call_yuv!(RGB24ToI420(
        src.as_ptr(),
        width * 3,
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width / 2,
        dst[y_size + uv_size..].as_mut_ptr(),
        width / 2,
        width,
        height,
    ))
}

/// Convert RGB24 to BGRA
pub fn rgb24_to_bgra(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;

    if src.len() < rgb24_size(w, h) || dst.len() < argb_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(RGB24ToARGB(
        src.as_ptr(),
        width * 3,
        dst.as_mut_ptr(),
        width * 4,
        width,
        height,
    ))
}

/// Convert BGR24 to BGRA
pub fn bgr24_to_bgra(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;

    if src.len() < rgb24_size(w, h) || dst.len() < argb_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(RAWToARGB(
        src.as_ptr(),
        width * 3,
        dst.as_mut_ptr(),
        width * 4,
        width,
        height,
    ))
}

// ============================================================================
// YUV to RGB conversions (for display/JPEG encoding)
// ============================================================================

/// Convert I420 to RGB24
pub fn i420_to_rgb24(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < i420_size(w, h) || dst.len() < rgb24_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(I420ToRGB24(
        src.as_ptr(),
        width,
        src[y_size..].as_ptr(),
        width / 2,
        src[y_size + uv_size..].as_ptr(),
        width / 2,
        dst.as_mut_ptr(),
        width * 3,
        width,
        height,
    ))
}

/// Convert I420 to BGRA
pub fn i420_to_bgra(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if src.len() < i420_size(w, h) || dst.len() < argb_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(I420ToARGB(
        src.as_ptr(),
        width,
        src[y_size..].as_ptr(),
        width / 2,
        src[y_size + uv_size..].as_ptr(),
        width / 2,
        dst.as_mut_ptr(),
        width * 4,
        width,
        height,
    ))
}

/// Convert NV12 to RGB24
pub fn nv12_to_rgb24(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;

    if src.len() < nv12_size(w, h) || dst.len() < rgb24_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(NV12ToRGB24(
        src.as_ptr(),
        width,
        src[y_size..].as_ptr(),
        width,
        dst.as_mut_ptr(),
        width * 3,
        width,
        height,
    ))
}

/// Convert NV12 to BGRA
pub fn nv12_to_bgra(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;

    if src.len() < nv12_size(w, h) || dst.len() < argb_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(NV12ToARGB(
        src.as_ptr(),
        width,
        src[y_size..].as_ptr(),
        width,
        dst.as_mut_ptr(),
        width * 4,
        width,
        height,
    ))
}

/// Convert YUYV to BGRA
pub fn yuy2_to_bgra(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;

    if src.len() < yuyv_size(w, h) || dst.len() < argb_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(YUY2ToARGB(
        src.as_ptr(),
        width * 2,
        dst.as_mut_ptr(),
        width * 4,
        width,
        height,
    ))
}

/// Convert YUYV to RGB24
pub fn yuy2_to_rgb24(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;

    if src.len() < yuyv_size(w, h) || dst.len() < rgb24_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    // libyuv doesn't have direct YUY2ToRGB24, use two-step conversion
    // First convert to BGRA, then to RGB24
    let mut bgra_buffer = vec![0u8; argb_size(w, h)];
    yuy2_to_bgra(src, &mut bgra_buffer, width, height)?;
    bgra_to_rgb24(&bgra_buffer, dst, width, height)
}

/// Convert UYVY to BGRA
pub fn uyvy_to_bgra(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;

    if src.len() < yuyv_size(w, h) || dst.len() < argb_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(UYVYToARGB(
        src.as_ptr(),
        width * 2,
        dst.as_mut_ptr(),
        width * 4,
        width,
        height,
    ))
}

/// Convert BGRA to RGB24
pub fn bgra_to_rgb24(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;

    if src.len() < argb_size(w, h) || dst.len() < rgb24_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(ARGBToRGB24(
        src.as_ptr(),
        width * 4,
        dst.as_mut_ptr(),
        width * 3,
        width,
        height,
    ))
}

/// Convert BGRA to BGR24
pub fn bgra_to_bgr24(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;

    if src.len() < argb_size(w, h) || dst.len() < rgb24_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(ARGBToRAW(
        src.as_ptr(),
        width * 4,
        dst.as_mut_ptr(),
        width * 3,
        width,
        height,
    ))
}

/// Convert RGB24 to NV12 (via two-step conversion: RGB24 → I420 → NV12)
pub fn rgb24_to_nv12(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;

    if src.len() < rgb24_size(w, h) || dst.len() < nv12_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    // Two-step conversion: RGB24 → I420 → NV12
    let mut i420_buffer = vec![0u8; i420_size(w, h)];
    rgb24_to_i420(src, &mut i420_buffer, width, height)?;
    i420_to_nv12(&i420_buffer, dst, width, height)
}

/// Convert BGR24 to NV12 (via two-step conversion: BGR24 → I420 → NV12)
pub fn bgr24_to_nv12(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;

    if src.len() < rgb24_size(w, h) || dst.len() < nv12_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    // Two-step conversion: BGR24 → I420 → NV12
    let mut i420_buffer = vec![0u8; i420_size(w, h)];
    bgr24_to_i420(src, &mut i420_buffer, width, height)?;
    i420_to_nv12(&i420_buffer, dst, width, height)
}

// ============================================================================
// MJPEG decoding
// ============================================================================

/// Decode MJPEG to I420
///
/// # Arguments
/// * `src` - Source MJPEG data
/// * `dst` - Destination I420 buffer
/// * `width` - Expected frame width
/// * `height` - Expected frame height
///
/// # Note
/// This function requires libyuv to be compiled with JPEG support
pub fn mjpeg_to_i420(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);

    if dst.len() < i420_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    if src.len() < 2 || src[0] != 0xFF || src[1] != 0xD8 {
        return Err(YuvError::InvalidMjpeg);
    }

    call_yuv!(MJPGToI420(
        src.as_ptr(),
        usize_to_size_t(src.len()),
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width / 2,
        dst[y_size + uv_size..].as_mut_ptr(),
        width / 2,
        width,
        height,
        width,
        height,
    ))
}

/// Decode MJPEG to NV12 (optimal for VAAPI)
pub fn mjpeg_to_nv12(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    if width % 2 != 0 || height % 2 != 0 {
        return Err(YuvError::InvalidDimensions);
    }

    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;

    if dst.len() < nv12_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    if src.len() < 2 || src[0] != 0xFF || src[1] != 0xD8 {
        return Err(YuvError::InvalidMjpeg);
    }

    call_yuv!(MJPGToNV12(
        src.as_ptr(),
        usize_to_size_t(src.len()),
        dst.as_mut_ptr(),
        width,
        dst[y_size..].as_mut_ptr(),
        width,
        width,
        height,
        width,
        height,
    ))
}

/// Decode MJPEG to BGRA
pub fn mjpeg_to_bgra(src: &[u8], dst: &mut [u8], width: i32, height: i32) -> Result<()> {
    let w = width as usize;
    let h = height as usize;

    if dst.len() < argb_size(w, h) {
        return Err(YuvError::BufferTooSmall);
    }

    if src.len() < 2 || src[0] != 0xFF || src[1] != 0xD8 {
        return Err(YuvError::InvalidMjpeg);
    }

    call_yuv!(MJPGToARGB(
        src.as_ptr(),
        usize_to_size_t(src.len()),
        dst.as_mut_ptr(),
        width * 4,
        width,
        height,
        width,
        height,
    ))
}

/// Get MJPEG frame dimensions without decoding
pub fn mjpeg_size(src: &[u8]) -> Result<(i32, i32)> {
    if src.len() < 2 || src[0] != 0xFF || src[1] != 0xD8 {
        return Err(YuvError::InvalidMjpeg);
    }

    let mut width: i32 = 0;
    let mut height: i32 = 0;

    let ret = unsafe { MJPGSize(src.as_ptr(), usize_to_size_t(src.len()), &mut width, &mut height) };

    if ret != 0 || width <= 0 || height <= 0 {
        return Err(YuvError::InvalidMjpeg);
    }

    Ok((width, height))
}

// ============================================================================
// Scaling
// ============================================================================

/// Scale I420 frame
///
/// # Arguments
/// * `src` - Source I420 data
/// * `src_width` - Source width
/// * `src_height` - Source height
/// * `dst` - Destination buffer
/// * `dst_width` - Destination width
/// * `dst_height` - Destination height
/// * `filter` - Filtering mode
pub fn i420_scale(
    src: &[u8],
    src_width: i32,
    src_height: i32,
    dst: &mut [u8],
    dst_width: i32,
    dst_height: i32,
    filter: FilterMode,
) -> Result<()> {
    let sw = src_width as usize;
    let sh = src_height as usize;
    let dw = dst_width as usize;
    let dh = dst_height as usize;

    let src_y_size = sw * sh;
    let src_uv_size = (sw / 2) * (sh / 2);
    let dst_y_size = dw * dh;
    let dst_uv_size = (dw / 2) * (dh / 2);

    if src.len() < i420_size(sw, sh) || dst.len() < i420_size(dw, dh) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(I420Scale(
        src.as_ptr(),
        src_width,
        src[src_y_size..].as_ptr(),
        src_width / 2,
        src[src_y_size + src_uv_size..].as_ptr(),
        src_width / 2,
        src_width,
        src_height,
        dst.as_mut_ptr(),
        dst_width,
        dst[dst_y_size..].as_mut_ptr(),
        dst_width / 2,
        dst[dst_y_size + dst_uv_size..].as_mut_ptr(),
        dst_width / 2,
        dst_width,
        dst_height,
        filter,
    ))
}

/// Scale NV12 frame
pub fn nv12_scale(
    src: &[u8],
    src_width: i32,
    src_height: i32,
    dst: &mut [u8],
    dst_width: i32,
    dst_height: i32,
    filter: FilterMode,
) -> Result<()> {
    let sw = src_width as usize;
    let sh = src_height as usize;
    let dw = dst_width as usize;
    let dh = dst_height as usize;

    let src_y_size = sw * sh;
    let dst_y_size = dw * dh;

    if src.len() < nv12_size(sw, sh) || dst.len() < nv12_size(dw, dh) {
        return Err(YuvError::BufferTooSmall);
    }

    call_yuv!(NV12Scale(
        src.as_ptr(),
        src_width,
        src[src_y_size..].as_ptr(),
        src_width,
        src_width,
        src_height,
        dst.as_mut_ptr(),
        dst_width,
        dst[dst_y_size..].as_mut_ptr(),
        dst_width,
        dst_width,
        dst_height,
        filter,
    ))
}

// ============================================================================
// High-level converter with buffer management
// ============================================================================

/// High-performance pixel format converter with pre-allocated buffers
///
/// This struct manages internal buffers to avoid repeated allocations
/// during continuous video processing.
pub struct Converter {
    width: i32,
    height: i32,
    nv12_buffer: Vec<u8>,
    i420_buffer: Vec<u8>,
}

impl Converter {
    /// Create a new converter for the given resolution
    pub fn new(width: i32, height: i32) -> Self {
        let w = width as usize;
        let h = height as usize;

        Self {
            width,
            height,
            nv12_buffer: vec![0u8; nv12_size(w, h)],
            i420_buffer: vec![0u8; i420_size(w, h)],
        }
    }

    /// Get frame dimensions
    pub fn dimensions(&self) -> (i32, i32) {
        (self.width, self.height)
    }

    /// Update resolution (reallocates buffers if needed)
    pub fn set_resolution(&mut self, width: i32, height: i32) {
        if width != self.width || height != self.height {
            let w = width as usize;
            let h = height as usize;
            self.width = width;
            self.height = height;
            self.nv12_buffer.resize(nv12_size(w, h), 0);
            self.i420_buffer.resize(i420_size(w, h), 0);
        }
    }

    /// Convert YUYV to NV12, returns reference to internal buffer
    pub fn yuy2_to_nv12(&mut self, src: &[u8]) -> Result<&[u8]> {
        yuy2_to_nv12(src, &mut self.nv12_buffer, self.width, self.height)?;
        Ok(&self.nv12_buffer)
    }

    /// Convert YUYV to I420, returns reference to internal buffer
    pub fn yuy2_to_i420(&mut self, src: &[u8]) -> Result<&[u8]> {
        yuy2_to_i420(src, &mut self.i420_buffer, self.width, self.height)?;
        Ok(&self.i420_buffer)
    }

    /// Convert UYVY to NV12, returns reference to internal buffer
    pub fn uyvy_to_nv12(&mut self, src: &[u8]) -> Result<&[u8]> {
        uyvy_to_nv12(src, &mut self.nv12_buffer, self.width, self.height)?;
        Ok(&self.nv12_buffer)
    }

    /// Decode MJPEG to NV12, returns reference to internal buffer
    pub fn mjpeg_to_nv12(&mut self, src: &[u8]) -> Result<&[u8]> {
        mjpeg_to_nv12(src, &mut self.nv12_buffer, self.width, self.height)?;
        Ok(&self.nv12_buffer)
    }

    /// Decode MJPEG to I420, returns reference to internal buffer
    pub fn mjpeg_to_i420(&mut self, src: &[u8]) -> Result<&[u8]> {
        mjpeg_to_i420(src, &mut self.i420_buffer, self.width, self.height)?;
        Ok(&self.i420_buffer)
    }

    /// Convert I420 to NV12, returns reference to internal buffer
    pub fn i420_to_nv12(&mut self, src: &[u8]) -> Result<&[u8]> {
        i420_to_nv12(src, &mut self.nv12_buffer, self.width, self.height)?;
        Ok(&self.nv12_buffer)
    }

    /// Convert NV12 to I420, returns reference to internal buffer
    pub fn nv12_to_i420(&mut self, src: &[u8]) -> Result<&[u8]> {
        nv12_to_i420(src, &mut self.i420_buffer, self.width, self.height)?;
        Ok(&self.i420_buffer)
    }

    /// Get NV12 buffer for direct writing
    pub fn nv12_buffer_mut(&mut self) -> &mut [u8] {
        &mut self.nv12_buffer
    }

    /// Get I420 buffer for direct writing
    pub fn i420_buffer_mut(&mut self) -> &mut [u8] {
        &mut self.i420_buffer
    }

    /// Get NV12 buffer reference
    pub fn nv12_buffer(&self) -> &[u8] {
        &self.nv12_buffer
    }

    /// Get I420 buffer reference
    pub fn i420_buffer(&self) -> &[u8] {
        &self.i420_buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_sizes() {
        assert_eq!(i420_size(1920, 1080), 1920 * 1080 * 3 / 2);
        assert_eq!(nv12_size(1920, 1080), 1920 * 1080 * 3 / 2);
        assert_eq!(yuyv_size(1920, 1080), 1920 * 1080 * 2);
        assert_eq!(rgb24_size(1920, 1080), 1920 * 1080 * 3);
        assert_eq!(argb_size(1920, 1080), 1920 * 1080 * 4);
    }

    #[test]
    fn test_invalid_dimensions() {
        let src = vec![0u8; 100];
        let mut dst = vec![0u8; 100];

        // Odd width should fail
        assert!(matches!(
            yuy2_to_nv12(&src, &mut dst, 3, 2),
            Err(YuvError::InvalidDimensions)
        ));

        // Odd height should fail
        assert!(matches!(
            yuy2_to_nv12(&src, &mut dst, 4, 3),
            Err(YuvError::InvalidDimensions)
        ));
    }

    #[test]
    fn test_buffer_too_small() {
        let src = vec![0u8; 10]; // Too small
        let mut dst = vec![0u8; nv12_size(4, 4)];

        assert!(matches!(
            yuy2_to_nv12(&src, &mut dst, 4, 4),
            Err(YuvError::BufferTooSmall)
        ));
    }

    #[test]
    fn test_converter() {
        let mut converter = Converter::new(4, 4);
        assert_eq!(converter.dimensions(), (4, 4));

        converter.set_resolution(8, 8);
        assert_eq!(converter.dimensions(), (8, 8));
        assert_eq!(converter.nv12_buffer().len(), nv12_size(8, 8));
    }
}
