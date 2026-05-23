//! Operations specific to UserPtr-type buffers.

use super::*;
use crate::bindings;

pub struct UserPtr;

impl Memory for UserPtr {
    const MEMORY_TYPE: MemoryType = MemoryType::UserPtr;
    type RawBacking = core::ffi::c_ulong;

    unsafe fn get_plane_buffer_backing(
        m: &bindings::v4l2_plane__bindgen_ty_1,
    ) -> &Self::RawBacking {
        &m.userptr
    }

    unsafe fn get_single_planar_buffer_backing(
        m: &bindings::v4l2_buffer__bindgen_ty_1,
    ) -> &Self::RawBacking {
        &m.userptr
    }

    unsafe fn get_plane_buffer_backing_mut(
        m: &mut bindings::v4l2_plane__bindgen_ty_1,
    ) -> &mut Self::RawBacking {
        &mut m.userptr
    }

    unsafe fn get_single_planar_buffer_backing_mut(
        m: &mut bindings::v4l2_buffer__bindgen_ty_1,
    ) -> &mut Self::RawBacking {
        &mut m.userptr
    }
}

impl Imported for UserPtr {}

/// Handle for a USERPTR plane. These buffers are backed by userspace-allocated
/// memory, which translates well into Rust's slice of `u8`s. Since slices also
/// carry size information, we know that we are not passing unallocated areas
/// of the address-space to the kernel.
///
/// USERPTR buffers have the particularity that the `length` field of `struct
/// v4l2_buffer` must be set before doing a `QBUF` ioctl. This handle struct
/// also takes care of that.
#[derive(Debug)]
pub struct UserPtrHandle<T: AsRef<[u8]> + Debug + Send + 'static>(pub T);

impl<T: AsRef<[u8]> + Debug + Send + Clone> Clone for UserPtrHandle<T> {
    fn clone(&self) -> Self {
        UserPtrHandle(self.0.clone())
    }
}

impl<T: AsRef<[u8]> + Debug + Send + 'static> AsRef<[u8]> for UserPtrHandle<T> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]> + Debug + Send> From<T> for UserPtrHandle<T> {
    fn from(buffer: T) -> Self {
        UserPtrHandle(buffer)
    }
}

impl<T: AsRef<[u8]> + Debug + Send + 'static> PlaneHandle for UserPtrHandle<T> {
    type Memory = UserPtr;

    fn fill_v4l2_plane(&self, plane: &mut bindings::v4l2_plane) {
        let slice = AsRef::<[u8]>::as_ref(&self.0);

        plane.m.userptr = slice.as_ptr() as std::os::raw::c_ulong;
        plane.length = slice.len() as u32;
    }
}
