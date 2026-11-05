use core::num::NonZeroUsize;
use std::{
    cmp::{max, min},
    ops::Deref,
    ptr::NonNull,
    slice,
};
use std::{ops::DerefMut, os::unix::io::AsFd};

use log::error;
use nix::{errno::Errno, libc::off_t, sys::mman};
use thiserror::Error;

pub struct PlaneMapping {
    // A mapping remains valid until we munmap it, that is, until the
    // PlaneMapping object is deleted. Hence the static lifetime.
    pub data: &'static mut [u8],

    start: usize,
    end: usize,
}

impl PlaneMapping {
    pub fn size(&self) -> usize {
        self.end - self.start
    }

    pub fn restrict(mut self, start: usize, end: usize) -> Self {
        self.start = max(self.start, start);
        self.end = min(self.end, end);

        self
    }
}

impl AsRef<[u8]> for PlaneMapping {
    fn as_ref(&self) -> &[u8] {
        &self.data[self.start..self.end]
    }
}

impl AsMut<[u8]> for PlaneMapping {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.data[self.start..self.end]
    }
}

impl Deref for PlaneMapping {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data[self.start..self.end]
    }
}

impl DerefMut for PlaneMapping {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data[self.start..self.end]
    }
}

impl Drop for PlaneMapping {
    fn drop(&mut self) {
        // Safe because the pointer and length were constructed in mmap() and
        // are always valid.
        unsafe {
            mman::munmap(
                NonNull::new_unchecked(self.data.as_mut_ptr().cast()),
                self.data.len(),
            )
        }
        .unwrap_or_else(|e| {
            error!("Error while unmapping plane: {}", e);
        });
    }
}

#[derive(Debug, Error)]
pub enum MmapError {
    #[error("provided length was 0")]
    ZeroLength,
    #[error("ioctl error: {0}")]
    IoctlError(#[from] Errno),
}

impl From<MmapError> for Errno {
    fn from(err: MmapError) -> Self {
        match err {
            MmapError::ZeroLength => Errno::EINVAL,
            MmapError::IoctlError(e) => e,
        }
    }
}

// TODO should be unsafe because the mapping can be used after a buffer is queued?
// Or not, since this cannot cause a crash...
pub fn mmap(fd: &impl AsFd, mem_offset: u32, length: u32) -> Result<PlaneMapping, MmapError> {
    let non_zero_length = NonZeroUsize::new(length as usize).ok_or(MmapError::ZeroLength)?;
    let data = unsafe {
        mman::mmap(
            None,
            non_zero_length,
            mman::ProtFlags::PROT_READ | mman::ProtFlags::PROT_WRITE,
            mman::MapFlags::MAP_SHARED,
            fd,
            mem_offset as off_t,
        )
    }?;

    Ok(PlaneMapping {
        // Safe because we know the pointer is valid and has enough data mapped
        // to cover the length.
        data: unsafe { slice::from_raw_parts_mut(data.as_ptr().cast(), length as usize) },
        start: 0,
        end: length as usize,
    })
}
