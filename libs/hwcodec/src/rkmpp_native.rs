//! Native Rockchip MPP C-API wrappers (zero-copy MJPEG->H.264 and RAM->H.264).
//! Used as a compatible, high-performance fallback when FFmpeg's DRM_PRIME/RGA
//! pipeline is unsupported (e.g. RK3399 Linux 4.19 BSPs).

use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr::NonNull;

unsafe extern "C" {
    fn rkmpp_native_mjpeg_h264_new(
        width: c_int,
        height: c_int,
        fps: c_int,
        kbps: c_int,
        gop: c_int,
    ) -> *mut c_void;
    fn rkmpp_native_mjpeg_h264_encode(
        pipeline: *mut c_void,
        jpeg_data: *const u8,
        jpeg_len: usize,
        pts_us: i64,
        force_idr: c_int,
        out_data: *mut *const u8,
        out_size: *mut usize,
        out_keyframe: *mut c_int,
    ) -> c_int;
    fn rkmpp_native_mjpeg_h264_reconfigure(pipeline: *mut c_void, kbps: c_int, gop: c_int)
        -> c_int;
    fn rkmpp_native_mjpeg_h264_free(pipeline: *mut c_void);

    fn rkmpp_native_enc_new(
        width: c_int,
        height: c_int,
        format: c_int,
        fps: c_int,
        kbps: c_int,
        gop: c_int,
    ) -> *mut c_void;
    fn rkmpp_native_enc_encode(
        encoder: *mut c_void,
        raw_data: *const u8,
        raw_len: usize,
        pts_us: i64,
        force_idr: c_int,
        out_data: *mut *const u8,
        out_size: *mut usize,
        out_keyframe: *mut c_int,
    ) -> c_int;
    fn rkmpp_native_enc_reconfigure(encoder: *mut c_void, kbps: c_int, gop: c_int) -> c_int;
    fn rkmpp_native_enc_free(encoder: *mut c_void);

    fn rkmpp_native_error() -> *const c_char;
}

pub fn last_error_message() -> String {
    unsafe {
        let ptr = rkmpp_native_error();
        if ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum NativeMppPixelFormat {
    Nv12 = 0,
    Yuyv422 = 1,
    Yuv420p = 2,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeMppConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub gop: u32,
}

pub struct NativeMppMjpegH264 {
    ctx: NonNull<c_void>,
}

unsafe impl Send for NativeMppMjpegH264 {}

impl NativeMppMjpegH264 {
    pub fn new(config: NativeMppConfig) -> Result<Self, String> {
        let ptr = unsafe {
            rkmpp_native_mjpeg_h264_new(
                config.width as c_int,
                config.height as c_int,
                config.fps as c_int,
                config.bitrate_kbps as c_int,
                config.gop.max(1) as c_int,
            )
        };
        Ok(Self {
            ctx: NonNull::new(ptr).ok_or_else(last_error_message)?,
        })
    }

    pub fn encode(
        &mut self,
        jpeg_data: &[u8],
        pts_ms: i64,
        force_idr: bool,
    ) -> Result<Option<(Vec<u8>, bool)>, String> {
        let mut out_data = std::ptr::null();
        let mut out_size = 0usize;
        let mut out_keyframe = 0 as c_int;
        let ret = unsafe {
            rkmpp_native_mjpeg_h264_encode(
                self.ctx.as_ptr(),
                jpeg_data.as_ptr(),
                jpeg_data.len(),
                pts_ms.saturating_mul(1000),
                force_idr as c_int,
                &mut out_data,
                &mut out_size,
                &mut out_keyframe,
            )
        };
        if ret != 0 {
            return Err(last_error_message());
        }
        if out_data.is_null() || out_size == 0 {
            return Ok(None);
        }
        let packet = unsafe { std::slice::from_raw_parts(out_data, out_size) }.to_vec();
        Ok(Some((packet, out_keyframe != 0)))
    }

    pub fn reconfigure(&mut self, kbps: u32, gop: u32) -> Result<(), String> {
        let ret = unsafe {
            rkmpp_native_mjpeg_h264_reconfigure(
                self.ctx.as_ptr(),
                kbps as c_int,
                gop.max(1) as c_int,
            )
        };
        if ret != 0 {
            return Err(last_error_message());
        }
        Ok(())
    }
}

impl Drop for NativeMppMjpegH264 {
    fn drop(&mut self) {
        unsafe { rkmpp_native_mjpeg_h264_free(self.ctx.as_ptr()) };
    }
}

pub struct NativeMppEncoder {
    ctx: NonNull<c_void>,
}

unsafe impl Send for NativeMppEncoder {}

impl NativeMppEncoder {
    pub fn new(config: NativeMppConfig, format: NativeMppPixelFormat) -> Result<Self, String> {
        let ptr = unsafe {
            rkmpp_native_enc_new(
                config.width as c_int,
                config.height as c_int,
                format as c_int,
                config.fps as c_int,
                config.bitrate_kbps as c_int,
                config.gop.max(1) as c_int,
            )
        };
        Ok(Self {
            ctx: NonNull::new(ptr).ok_or_else(last_error_message)?,
        })
    }

    pub fn encode(
        &mut self,
        raw_data: &[u8],
        pts_ms: i64,
        force_idr: bool,
    ) -> Result<Option<(Vec<u8>, bool)>, String> {
        let mut out_data = std::ptr::null();
        let mut out_size = 0usize;
        let mut out_keyframe = 0 as c_int;
        let ret = unsafe {
            rkmpp_native_enc_encode(
                self.ctx.as_ptr(),
                raw_data.as_ptr(),
                raw_data.len(),
                pts_ms.saturating_mul(1000),
                force_idr as c_int,
                &mut out_data,
                &mut out_size,
                &mut out_keyframe,
            )
        };
        if ret != 0 {
            return Err(last_error_message());
        }
        if out_data.is_null() || out_size == 0 {
            return Ok(None);
        }
        let packet = unsafe { std::slice::from_raw_parts(out_data, out_size) }.to_vec();
        Ok(Some((packet, out_keyframe != 0)))
    }

    pub fn reconfigure(&mut self, kbps: u32, gop: u32) -> Result<(), String> {
        let ret = unsafe {
            rkmpp_native_enc_reconfigure(
                self.ctx.as_ptr(),
                kbps as c_int,
                gop.max(1) as c_int,
            )
        };
        if ret != 0 {
            return Err(last_error_message());
        }
        Ok(())
    }
}

impl Drop for NativeMppEncoder {
    fn drop(&mut self) {
        unsafe { rkmpp_native_enc_free(self.ctx.as_ptr()) };
    }
}
