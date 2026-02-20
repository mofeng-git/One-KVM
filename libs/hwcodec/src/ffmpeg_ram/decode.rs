use crate::{
    ffmpeg::{init_av_log, AVPixelFormat},
    ffmpeg_ram::{
        ffmpeg_ram_decode, ffmpeg_ram_free_decoder, ffmpeg_ram_last_error, ffmpeg_ram_new_decoder,
    },
};
use std::{
    ffi::{c_void, CString},
    os::raw::c_int,
    slice,
};

#[derive(Debug, Clone, PartialEq)]
pub struct DecodeContext {
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub sw_pixfmt: AVPixelFormat,
    pub thread_count: i32,
}

pub struct DecodeFrame {
    pub data: Vec<u8>,
    pub width: i32,
    pub height: i32,
    pub pixfmt: AVPixelFormat,
}

pub struct Decoder {
    codec: *mut c_void,
    frames: *mut Vec<DecodeFrame>,
    pub ctx: DecodeContext,
}

// Safety: Decoder is only accessed through higher-level synchronization
// (a tokio::Mutex in the video pipeline). It is never accessed concurrently,
// but may be moved across threads; the underlying FFmpeg RAM decoder state
// is thread-confined per instance, so Send (but not Sync) is acceptable.
unsafe impl Send for Decoder {}

impl Decoder {
    pub fn new(ctx: DecodeContext) -> Result<Self, ()> {
        init_av_log();
        unsafe {
            let codec = ffmpeg_ram_new_decoder(
                CString::new(ctx.name.as_str()).map_err(|_| ())?.as_ptr(),
                ctx.width,
                ctx.height,
                ctx.sw_pixfmt as c_int,
                ctx.thread_count,
                Some(Decoder::callback),
            );
            if codec.is_null() {
                let msg = last_error_message();
                if !msg.is_empty() {
                    log::error!("ffmpeg_ram_new_decoder failed: {}", msg);
                }
                return Err(());
            }
            Ok(Decoder {
                codec,
                frames: Box::into_raw(Box::new(Vec::<DecodeFrame>::new())),
                ctx,
            })
        }
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<&mut Vec<DecodeFrame>, i32> {
        unsafe {
            (&mut *self.frames).clear();
            let ret = ffmpeg_ram_decode(
                self.codec,
                data.as_ptr(),
                data.len() as c_int,
                self.frames as *const _ as *const c_void,
            );
            if ret != 0 {
                let msg = last_error_message();
                if !msg.is_empty() {
                    log::error!("ffmpeg_ram_decode failed: {}", msg);
                }
                return Err(ret);
            }
            Ok(&mut *self.frames)
        }
    }

    extern "C" fn callback(
        data: *const u8,
        size: c_int,
        width: c_int,
        height: c_int,
        pixfmt: c_int,
        obj: *const c_void,
    ) {
        unsafe {
            let frames = &mut *(obj as *mut Vec<DecodeFrame>);
            frames.push(DecodeFrame {
                data: slice::from_raw_parts(data, size as usize).to_vec(),
                width,
                height,
                pixfmt: std::mem::transmute::<i32, AVPixelFormat>(pixfmt),
            });
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            ffmpeg_ram_free_decoder(self.codec);
            drop(Box::from_raw(self.frames));
        }
    }
}

fn last_error_message() -> String {
    unsafe {
        let ptr = ffmpeg_ram_last_error();
        if ptr.is_null() {
            return String::new();
        }
        let cstr = std::ffi::CStr::from_ptr(ptr);
        cstr.to_string_lossy().to_string()
    }
}
