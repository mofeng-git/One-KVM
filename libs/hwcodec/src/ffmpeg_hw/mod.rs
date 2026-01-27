#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{
    ffi::{CStr, CString},
    os::raw::c_int,
};

include!(concat!(env!("OUT_DIR"), "/ffmpeg_hw_ffi.rs"));

#[derive(Debug, Clone)]
pub struct HwMjpegH26xConfig {
    pub decoder: String,
    pub encoder: String,
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub bitrate_kbps: i32,
    pub gop: i32,
    pub thread_count: i32,
}

pub struct HwMjpegH26xPipeline {
    ctx: *mut FfmpegHwMjpegH26x,
    config: HwMjpegH26xConfig,
}

unsafe impl Send for HwMjpegH26xPipeline {}

impl HwMjpegH26xPipeline {
    pub fn new(config: HwMjpegH26xConfig) -> Result<Self, String> {
        unsafe {
            let dec = CString::new(config.decoder.as_str()).map_err(|_| "decoder name invalid".to_string())?;
            let enc = CString::new(config.encoder.as_str()).map_err(|_| "encoder name invalid".to_string())?;
            let ctx = ffmpeg_hw_mjpeg_h26x_new(
                dec.as_ptr(),
                enc.as_ptr(),
                config.width,
                config.height,
                config.fps,
                config.bitrate_kbps,
                config.gop,
                config.thread_count,
            );
            if ctx.is_null() {
                return Err(last_error_message());
            }
            Ok(Self { ctx, config })
        }
    }

    pub fn encode(&mut self, data: &[u8], pts_ms: i64) -> Result<Option<(Vec<u8>, bool)>, String> {
        unsafe {
            let mut out_data: *mut u8 = std::ptr::null_mut();
            let mut out_len: c_int = 0;
            let mut out_key: c_int = 0;
            let ret = ffmpeg_hw_mjpeg_h26x_encode(
                self.ctx,
                data.as_ptr(),
                data.len() as c_int,
                pts_ms,
                &mut out_data,
                &mut out_len,
                &mut out_key,
            );
            if ret < 0 {
                return Err(last_error_message());
            }
            if out_data.is_null() || out_len == 0 {
                return Ok(None);
            }
            let slice = std::slice::from_raw_parts(out_data, out_len as usize);
            let mut vec = Vec::with_capacity(slice.len());
            vec.extend_from_slice(slice);
            ffmpeg_hw_packet_free(out_data);
            Ok(Some((vec, out_key != 0)))
        }
    }

    pub fn reconfigure(&mut self, bitrate_kbps: i32, gop: i32) -> Result<(), String> {
        unsafe {
            let ret = ffmpeg_hw_mjpeg_h26x_reconfigure(self.ctx, bitrate_kbps, gop);
            if ret != 0 {
                return Err(last_error_message());
            }
            self.config.bitrate_kbps = bitrate_kbps;
            self.config.gop = gop;
            Ok(())
        }
    }

    pub fn request_keyframe(&mut self) {
        unsafe {
            let _ = ffmpeg_hw_mjpeg_h26x_request_keyframe(self.ctx);
        }
    }
}

impl Drop for HwMjpegH26xPipeline {
    fn drop(&mut self) {
        unsafe {
            ffmpeg_hw_mjpeg_h26x_free(self.ctx);
        }
        self.ctx = std::ptr::null_mut();
    }
}

pub fn last_error_message() -> String {
    unsafe {
        let ptr = ffmpeg_hw_last_error();
        if ptr.is_null() {
            return String::new();
        }
        let cstr = CStr::from_ptr(ptr);
        cstr.to_string_lossy().to_string()
    }
}
