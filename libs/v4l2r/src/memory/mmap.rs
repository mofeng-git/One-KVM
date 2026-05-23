//! Operations specific to MMAP-type buffers.
use super::*;
use crate::{bindings, ioctl};
use std::fmt::Debug;
use std::os::fd::AsFd;

#[derive(Default)]
pub struct Mmap;

impl Memory for Mmap {
    const MEMORY_TYPE: MemoryType = MemoryType::Mmap;
    type RawBacking = u32;

    unsafe fn get_plane_buffer_backing(
        m: &bindings::v4l2_plane__bindgen_ty_1,
    ) -> &Self::RawBacking {
        &m.mem_offset
    }

    unsafe fn get_single_planar_buffer_backing(
        m: &bindings::v4l2_buffer__bindgen_ty_1,
    ) -> &Self::RawBacking {
        &m.offset
    }

    unsafe fn get_plane_buffer_backing_mut(
        m: &mut bindings::v4l2_plane__bindgen_ty_1,
    ) -> &mut Self::RawBacking {
        &mut m.mem_offset
    }

    unsafe fn get_single_planar_buffer_backing_mut(
        m: &mut bindings::v4l2_buffer__bindgen_ty_1,
    ) -> &mut Self::RawBacking {
        &mut m.offset
    }
}

impl SelfBacked for Mmap {}

/// Dummy handle for a MMAP plane, to use with APIs that require handles. MMAP
/// buffers are backed by the device, and thus we don't need to attach any extra
/// information to them.
#[derive(Default, Debug, Clone)]
pub struct MmapHandle;

// There is no information to fill with MMAP buffers ; the index is enough.
impl PlaneHandle for MmapHandle {
    type Memory = Mmap;

    fn fill_v4l2_plane(&self, _plane: &mut bindings::v4l2_plane) {}
}

impl Mappable for MmapHandle {
    fn map<D: AsFd>(device: &D, plane_info: &QueryBufPlane) -> Option<PlaneMapping> {
        ioctl::mmap(device, plane_info.mem_offset, plane_info.length).ok()
    }
}
