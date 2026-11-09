#[cfg(feature = "fixtures")]
use anyhow::Context;
use anyhow::{bail, Result};
use std::ffi::c_void;
#[cfg(feature = "fixtures")]
use std::{
    ffi::{c_char, CString},
    path::Path,
};

unsafe extern "C" {
    fn af_create() -> *mut c_void;
    fn af_destroy(decoder: *mut c_void);
    fn af_decode(
        decoder: *mut c_void,
        jpeg: *const u8,
        size: usize,
        callback: extern "C" fn(*const u8, usize, *mut c_void),
        context: *mut c_void,
        jpeg_ms: *mut f64,
        qr_ms: *mut f64,
    ) -> i32;
    #[cfg(feature = "fixtures")]
    fn af_write_gray_jpeg(gray: *const u8, w: i32, h: i32, path: *const c_char) -> i32;
}

pub struct Decoder {
    raw: *mut c_void,
}
impl Decoder {
    pub fn new() -> Result<Self> {
        let ptr = unsafe { af_create() };
        if ptr.is_null() {
            bail!("Cannot create JPEG decoder");
        }
        Ok(Self { raw: ptr })
    }
    pub fn decode(&mut self, jpeg: &[u8]) -> Result<(Vec<Vec<u8>>, f64, f64)> {
        extern "C" fn payload(data: *const u8, len: usize, context: *mut c_void) {
            // Native code owns data until this callback returns. Copy just the QR bytes.
            if len <= 4096 {
                unsafe {
                    let results = &mut *(context as *mut Vec<Vec<u8>>);
                    if results.len() < 64 {
                        results.push(std::slice::from_raw_parts(data, len).to_vec());
                    }
                }
            }
        }
        let mut results = Vec::<Vec<u8>>::new();
        let (mut jpeg_ms, mut qr_ms) = (0.0, 0.0);
        let rc = unsafe {
            af_decode(
                self.raw,
                jpeg.as_ptr(),
                jpeg.len(),
                payload,
                &mut results as *mut _ as *mut c_void,
                &mut jpeg_ms,
                &mut qr_ms,
            )
        };
        if rc < 0 {
            bail!("JPEG/QR decoder rejected frame ({rc})");
        }
        Ok((results, jpeg_ms, qr_ms))
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { af_destroy(self.raw) }
    }
}

#[cfg(feature = "fixtures")]
pub fn write_gray_jpeg(gray: &[u8], width: i32, height: i32, path: &Path) -> Result<()> {
    anyhow::ensure!(
        width > 0 && height > 0 && gray.len() == width as usize * height as usize,
        "Invalid fixture geometry"
    );
    let path = CString::new(path.to_str().context("Non-UTF8 fixture path")?)?;
    if unsafe { af_write_gray_jpeg(gray.as_ptr(), width, height, path.as_ptr()) } != 0 {
        bail!("JPEG fixture encoding failed");
    }
    Ok(())
}
