//! Synchronous RKMPP encoder for pre-exported V4L2 DMA buffers.
//! Unlike the byte-slice encoder this never reads raw pixels on the CPU.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::os::fd::{AsRawFd, OwnedFd};
use std::ptr::NonNull;

unsafe extern "C" {
    fn rkmpp_dma_new(
        width: c_int,
        height: c_int,
        stride: c_int,
        format: c_int,
        codec: c_int,
        fps: c_int,
        kbps: c_int,
        gop: c_int,
        fds: *const c_int,
        sizes: *const usize,
        count: usize,
    ) -> *mut c_void;
    fn rkmpp_dma_encode(
        encoder: *mut c_void,
        index: usize,
        bytes_used: usize,
        fresh_fd: c_int,
        pts_us: i64,
        force_idr: c_int,
        data: *mut *const u8,
        size: *mut usize,
    ) -> c_int;
    fn rkmpp_dma_reconfigure(encoder: *mut c_void, kbps: c_int, gop: c_int) -> c_int;
    fn rkmpp_dma_free(encoder: *mut c_void);
    fn rkmpp_dma_error() -> *const c_char;
}

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum DmaFormat {
    Nv12 = 0,
    Bgr24 = 1,
    Yuyv = 2,
    Rgb24 = 3,
    Mjpeg = 4,
}

pub struct DmaEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: DmaFormat,
    pub hevc: bool,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub gop: u32,
}

pub struct DmaEncoder {
    ctx: NonNull<c_void>,
    // Export FDs remain open until AFTER mpp_destroy and imported buffer release.
    _buffers: Vec<(OwnedFd, usize)>,
    // Keep refreshed exports alive until native replacement/release has ended
    // all references to the previous import. At most one FD per capture slot.
    fresh_buffers: Vec<Option<OwnedFd>>,
}

// Exclusive ownership: the context can move between threads but all calls are sequential.
unsafe impl Send for DmaEncoder {}

fn last_error() -> String {
    unsafe {
        CStr::from_ptr(rkmpp_dma_error())
            .to_string_lossy()
            .into_owned()
    }
}

impl DmaEncoder {
    pub fn new(config: DmaEncoderConfig, buffers: Vec<(OwnedFd, usize)>) -> Result<Self, String> {
        let fds: Vec<_> = buffers.iter().map(|(fd, _)| fd.as_raw_fd()).collect();
        let sizes: Vec<_> = buffers.iter().map(|(_, size)| *size).collect();
        for value in [
            config.width,
            config.height,
            config.stride,
            config.fps,
            config.bitrate_kbps,
            config.gop,
        ] {
            if value > c_int::MAX as u32 {
                return Err("DMA encoder parameter overflow".into());
            }
        }
        let ptr = unsafe {
            rkmpp_dma_new(
                config.width as _,
                config.height as _,
                config.stride as _,
                config.format as c_int,
                config.hevc as _,
                config.fps as _,
                config.bitrate_kbps as _,
                config.gop as _,
                fds.as_ptr(),
                sizes.as_ptr(),
                buffers.len(),
            )
        };
        Ok(Self {
            ctx: NonNull::new(ptr).ok_or_else(last_error)?,
            fresh_buffers: (0..buffers.len()).map(|_| None).collect(),
            _buffers: buffers,
        })
    }

    /// # Safety
    /// The indexed buffer must be dequeued and exclusively leased to this call.
    /// Do not requeue/write it until this function returns. On failure native MPP
    /// is synchronously destroyed before returning, ending all input access.
    /// `bytes_used` is the actual DQBUF payload length, not the buffer capacity.
    /// A refreshed FD, if supplied, must refer to the same leased capture slot
    /// with the capacity registered at construction. Ownership is retained here.
    pub unsafe fn encode(
        &mut self,
        index: usize,
        bytes_used: usize,
        fresh_fd: Option<OwnedFd>,
        pts_ms: i64,
        force_idr: bool,
    ) -> Result<Vec<u8>, String> {
        let mut data = std::ptr::null();
        let mut size = 0;
        if index >= self.fresh_buffers.len() {
            return Err("Invalid DMA capture index".into());
        }
        let ret = unsafe {
            rkmpp_dma_encode(
                self.ctx.as_ptr(),
                index,
                bytes_used,
                fresh_fd.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                pts_ms.saturating_mul(1000),
                force_idr as _,
                &mut data,
                &mut size,
            )
        };
        if fresh_fd.is_some() {
            // Native has now released the previous import, or destroyed both
            // hardware contexts on error. Only now may its old FD be closed.
            self.fresh_buffers[index] = fresh_fd;
        }
        if ret != 0 {
            return Err(last_error());
        }
        if data.is_null() || size == 0 {
            return Err("Empty RKMPP DMA packet".into());
        }
        // Copy only the compressed output, releasing the driver's packet promptly
        // regardless of how long a network subscriber retains its Bytes.
        Ok(unsafe { std::slice::from_raw_parts(data, size) }.to_vec())
    }

    pub fn reconfigure(&mut self, kbps: u32, gop: u32) -> Result<(), String> {
        if kbps > c_int::MAX as u32 || gop > c_int::MAX as u32 {
            return Err("DMA encoder parameter overflow".into());
        }
        if unsafe { rkmpp_dma_reconfigure(self.ctx.as_ptr(), kbps as _, gop as _) } != 0 {
            return Err(last_error());
        }
        Ok(())
    }
}

impl Drop for DmaEncoder {
    fn drop(&mut self) {
        unsafe { rkmpp_dma_free(self.ctx.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_discriminants_match_native_abi() {
        assert_eq!(DmaFormat::Nv12 as c_int, 0);
        assert_eq!(DmaFormat::Bgr24 as c_int, 1);
        assert_eq!(DmaFormat::Yuyv as c_int, 2);
        assert_eq!(DmaFormat::Rgb24 as c_int, 3);
        assert_eq!(DmaFormat::Mjpeg as c_int, 4);
    }
}
