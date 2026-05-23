//! Abstracts the different kinds of backing memory (`MMAP`, `USERPTR`,
//! `DMABUF`) supported by V4L2.
//!
//! V4L2 allows to use either memory that is provided by the device itself
//! (MMAP) or memory imported via user allocation (USERPTR) or the dma-buf
//! subsystem (DMABUF). This results in 2 very different behaviors and 3 memory
//! types that we need to model.
//!
//! The `Memory` trait represents these memory types and is thus implemented
//! by exacly 3 types: `MMAP`, `UserPtr`, and `DMABuf`. These types do very
//! little apart from providing a constant with the corresponding V4L2 memory
//! type they model, and implement the `SelfBacked` (for MMAP) or `Imported`
//! (for `UserPtr` and `DMABuf`) traits to indicate where their memory comes
//! from.
//!
//! The `PlaneHandle` trait is used by types which can bind to one of these
//! memory types, i.e. a type that can represent a single memory plane of a
//! buffer. For `MMAP` memory this is a void type (since `MMAP` provides its
//! own memory). `UserPtr`, a `Vec<u8>` can adequately be used as backing
//! memory, and for `DMABuf` we will use a file descriptor. For handles that
//! can be mapped into the user address-space (and indeed for `MMAP` this is
//! the only way to access the memory), the `Mappable` trait can be implemented.
//!
//! The set of handles that make all the planes for a given buffer is
//! represented by the `BufferHandles` trait. This trait is more abstract since
//! we may want to decide at runtime the kind of memory we want to use ;
//! therefore this trait does not have any particular kind of memory attached to
//! it. `PrimitiveBufferHandles` is used to represent plane handles which memory
//! type is known at compilation time, and thus includes a reference to a
//! `PlaneHandle` type and by transition its `Memory` type.
mod dmabuf;
mod mmap;
mod userptr;

pub use dmabuf::*;
pub use mmap::*;
pub use userptr::*;

use crate::{
    bindings::{self, v4l2_buffer__bindgen_ty_1, v4l2_plane__bindgen_ty_1},
    ioctl::{PlaneMapping, QueryBufPlane},
};
use enumn::N;
use std::os::unix::io::AsFd;
use std::{fmt::Debug, ops::Deref};

/// All the supported V4L2 memory types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, N)]
#[repr(u32)]
pub enum MemoryType {
    Mmap = bindings::v4l2_memory_V4L2_MEMORY_MMAP,
    UserPtr = bindings::v4l2_memory_V4L2_MEMORY_USERPTR,
    Overlay = bindings::v4l2_memory_V4L2_MEMORY_OVERLAY,
    DmaBuf = bindings::v4l2_memory_V4L2_MEMORY_DMABUF,
}

/// Trait describing a memory type that can be used to back V4L2 buffers.
pub trait Memory: 'static {
    /// The memory type represented.
    const MEMORY_TYPE: MemoryType;
    /// The final type of the memory backing information in `struct v4l2_buffer` or `struct
    /// v4l2_plane`.
    type RawBacking;

    /// Returns a reference to the memory backing information for `m` that is relevant for this
    /// memory type.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `m` indeed belongs to a buffer of this memory type.
    unsafe fn get_plane_buffer_backing(m: &bindings::v4l2_plane__bindgen_ty_1)
        -> &Self::RawBacking;

    /// Returns a reference to the memory backing information for `m` that is relevant for this memory type.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `m` indeed belongs to a buffer of this memory type.
    unsafe fn get_single_planar_buffer_backing(
        m: &bindings::v4l2_buffer__bindgen_ty_1,
    ) -> &Self::RawBacking;

    /// Returns a mutable reference to the memory backing information for `m` that is relevant for
    /// this memory type.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `m` indeed belongs to a buffer of this memory type.
    unsafe fn get_plane_buffer_backing_mut(
        m: &mut bindings::v4l2_plane__bindgen_ty_1,
    ) -> &mut Self::RawBacking;

    /// Returns a mutable reference to the memory backing information for `m` that is relevant for
    /// this memory type.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `m` indeed belongs to a buffer of this memory type.
    unsafe fn get_single_planar_buffer_backing_mut(
        m: &mut bindings::v4l2_buffer__bindgen_ty_1,
    ) -> &mut Self::RawBacking;
}

/// Trait for memory types that provide their own memory, i.e. MMAP.
pub trait SelfBacked: Memory + Default {}

/// Trait for memory types to which external memory must be attached to, i.e. UserPtr and
/// DMABuf.
pub trait Imported: Memory {}

/// Trait for a handle that represents actual data for a single place. A buffer
/// will have as many of these as it has planes.
pub trait PlaneHandle: Debug + Send + 'static {
    /// The kind of memory the handle attaches to.
    type Memory: Memory;

    /// Fill a plane of a multi-planar V4L2 buffer with the handle's information.
    fn fill_v4l2_plane(&self, plane: &mut bindings::v4l2_plane);
}

// Trait for plane handles that provide access to their content through a map()
// method (typically, MMAP buffers).
pub trait Mappable: PlaneHandle {
    /// Return a `PlaneMapping` enabling access to the memory of this handle.
    fn map<D: AsFd>(device: &D, plane_info: &QueryBufPlane) -> Option<PlaneMapping>;
}

/// Trait for structures providing all the handles of a single buffer.
pub trait BufferHandles: Send + Debug + 'static {
    /// Enumeration of all the `MemoryType` supported by this type. Typically
    /// a subset of `MemoryType` or `MemoryType` itself.
    type SupportedMemoryType: Into<MemoryType> + Send + Clone + Copy;

    /// Number of planes.
    fn len(&self) -> usize;
    /// Fill a plane of a multi-planar V4L2 buffer with the `index` handle's information.
    fn fill_v4l2_plane(&self, index: usize, plane: &mut bindings::v4l2_plane);

    /// Returns true if there are no handles here (unlikely).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Implementation of `BufferHandles` for all indexables of `PlaneHandle` (e.g. [`std::vec::Vec`]).
///
/// This is The simplest way to use primitive handles.
impl<P, Q> BufferHandles for Q
where
    P: PlaneHandle,
    Q: Send + Debug + 'static + Deref<Target = [P]>,
{
    type SupportedMemoryType = MemoryType;

    fn len(&self) -> usize {
        self.deref().len()
    }

    fn fill_v4l2_plane(&self, index: usize, plane: &mut bindings::v4l2_plane) {
        self.deref()[index].fill_v4l2_plane(plane);
    }
}

/// Trait for plane handles for which the final memory type is known at compile
/// time.
pub trait PrimitiveBufferHandles: BufferHandles {
    type HandleType: PlaneHandle;
    const MEMORY_TYPE: Self::SupportedMemoryType;
}

/// Implementation of `PrimitiveBufferHandles` for all indexables of `PlaneHandle` (e.g.
/// [`std::vec::Vec`]).
impl<P, Q> PrimitiveBufferHandles for Q
where
    P: PlaneHandle,
    Q: Send + Debug + 'static + Deref<Target = [P]>,
{
    type HandleType = P;
    const MEMORY_TYPE: Self::SupportedMemoryType = P::Memory::MEMORY_TYPE;
}

/// Conversion from `v4l2_buffer`'s backing information to `v4l2_plane`'s.
impl From<(&v4l2_buffer__bindgen_ty_1, MemoryType)> for v4l2_plane__bindgen_ty_1 {
    fn from((m, memory): (&v4l2_buffer__bindgen_ty_1, MemoryType)) -> Self {
        match memory {
            MemoryType::Mmap => v4l2_plane__bindgen_ty_1 {
                // Safe because the buffer type is determined to be MMAP.
                mem_offset: unsafe { m.offset },
            },
            MemoryType::UserPtr => v4l2_plane__bindgen_ty_1 {
                // Safe because the buffer type is determined to be USERPTR.
                userptr: unsafe { m.userptr },
            },
            MemoryType::DmaBuf => v4l2_plane__bindgen_ty_1 {
                // Safe because the buffer type is determined to be DMABUF.
                fd: unsafe { m.fd },
            },
            MemoryType::Overlay => Default::default(),
        }
    }
}

/// Conversion from `v4l2_plane`'s backing information to `v4l2_buffer`'s.
impl From<(&v4l2_plane__bindgen_ty_1, MemoryType)> for v4l2_buffer__bindgen_ty_1 {
    fn from((m, memory): (&v4l2_plane__bindgen_ty_1, MemoryType)) -> Self {
        match memory {
            MemoryType::Mmap => v4l2_buffer__bindgen_ty_1 {
                // Safe because the buffer type is determined to be MMAP.
                offset: unsafe { m.mem_offset },
            },
            MemoryType::UserPtr => v4l2_buffer__bindgen_ty_1 {
                // Safe because the buffer type is determined to be USERPTR.
                userptr: unsafe { m.userptr },
            },
            MemoryType::DmaBuf => v4l2_buffer__bindgen_ty_1 {
                // Safe because the buffer type is determined to be DMABUF.
                fd: unsafe { m.fd },
            },
            MemoryType::Overlay => Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bindings::v4l2_buffer__bindgen_ty_1;
    use crate::bindings::v4l2_plane__bindgen_ty_1;
    use crate::memory::MemoryType;

    #[test]
    // Purpose of this test is dubious as the members are overlapping anyway.
    fn plane_m_to_buffer_m() {
        let plane_m = v4l2_plane__bindgen_ty_1 {
            mem_offset: 0xfeedc0fe,
        };
        assert_eq!(
            unsafe { v4l2_buffer__bindgen_ty_1::from((&plane_m, MemoryType::Mmap)).offset },
            0xfeedc0fe
        );

        let plane_m = v4l2_plane__bindgen_ty_1 {
            userptr: 0xfeedc0fe,
        };
        assert_eq!(
            unsafe { v4l2_buffer__bindgen_ty_1::from((&plane_m, MemoryType::UserPtr)).userptr },
            0xfeedc0fe
        );

        let plane_m = v4l2_plane__bindgen_ty_1 { fd: 0x76543210 };
        assert_eq!(
            unsafe { v4l2_buffer__bindgen_ty_1::from((&plane_m, MemoryType::DmaBuf)).fd },
            0x76543210
        );
    }

    #[test]
    // Purpose of this test is dubious as the members are overlapping anyway.
    fn buffer_m_to_plane_m() {
        let buffer_m = v4l2_buffer__bindgen_ty_1 { offset: 0xfeedc0fe };
        assert_eq!(
            unsafe { v4l2_plane__bindgen_ty_1::from((&buffer_m, MemoryType::Mmap)).mem_offset },
            0xfeedc0fe
        );

        let buffer_m = v4l2_buffer__bindgen_ty_1 {
            userptr: 0xfeedc0fe,
        };
        assert_eq!(
            unsafe { v4l2_plane__bindgen_ty_1::from((&buffer_m, MemoryType::UserPtr)).userptr },
            0xfeedc0fe
        );

        let buffer_m = v4l2_buffer__bindgen_ty_1 { fd: 0x76543210 };
        assert_eq!(
            unsafe { v4l2_plane__bindgen_ty_1::from((&buffer_m, MemoryType::DmaBuf)).fd },
            0x76543210
        );
    }
}
