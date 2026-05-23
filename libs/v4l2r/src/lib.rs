//! This library provides the V4L2 pieces One-KVM needs for video capture:
//!
//! * The `ioctl` module provides direct, thin wrappers over the V4L2 ioctls
//!   with added safety. Note that "safety" here is in terms of memory safety:
//!   this layer won't guard against passing invalid data that the ioctls will
//!   reject - it just makes sure that data passed from and to the kernel can
//!   be accessed safely. Since this is a 1:1 mapping over the V4L2 ioctls,
//!   working at this level is a bit laborious, although more comfortable than
//!   doing the same in C.
//!
//! The upstream v4l2r crate also contains high-level decoder/encoder and C FFI
//! layers. This vendored copy intentionally excludes those pieces and keeps the
//! capture-oriented ioctl/memory surface only.
//!
#[doc(hidden)]
pub mod bindings;
pub mod ioctl;
pub mod memory;

// This can be needed to match nix errors that we expose.
pub use nix;

use std::convert::TryFrom;
use std::fmt;
use std::fmt::{Debug, Display};

use enumn::N;
use thiserror::Error;

// The goal of this library is to provide two layers of abstraction:
// ioctl: direct, safe counterparts of the V4L2 ioctls.
// device/queue/buffer: higher abstraction, still mapping to core V4L2 mechanics.

/// Possible directions for the queue
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum QueueDirection {
    Output,
    Capture,
}

/// Possible classes for this queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum QueueClass {
    Video,
    Vbi,
    SlicedVbi,
    VideoOverlay,
    VideoMplane,
    Sdr,
    Meta,
}

/// Types of queues currently supported by this library.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, N)]
#[repr(u32)]
pub enum QueueType {
    VideoCapture = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE,
    VideoOutput = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_OUTPUT,
    VideoOverlay = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_OVERLAY,
    VbiCapture = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VBI_CAPTURE,
    VbiOutput = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VBI_OUTPUT,
    SlicedVbiCapture = bindings::v4l2_buf_type_V4L2_BUF_TYPE_SLICED_VBI_CAPTURE,
    SlicedVbiOutput = bindings::v4l2_buf_type_V4L2_BUF_TYPE_SLICED_VBI_OUTPUT,
    VideoOutputOverlay = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_OUTPUT_OVERLAY,
    VideoCaptureMplane = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE,
    VideoOutputMplane = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_OUTPUT_MPLANE,
    SdrCapture = bindings::v4l2_buf_type_V4L2_BUF_TYPE_SDR_CAPTURE,
    SdrOutput = bindings::v4l2_buf_type_V4L2_BUF_TYPE_SDR_OUTPUT,
    MetaCapture = bindings::v4l2_buf_type_V4L2_BUF_TYPE_META_CAPTURE,
    MetaOutput = bindings::v4l2_buf_type_V4L2_BUF_TYPE_META_OUTPUT,
}

impl QueueType {
    /// Returns the queue corresponding to the passed `direction` and `class`.
    pub fn from_dir_and_class(direction: QueueDirection, class: QueueClass) -> Self {
        match (direction, class) {
            (QueueDirection::Capture, QueueClass::Video) => Self::VideoCapture,
            (QueueDirection::Output, QueueClass::Video) => Self::VideoOutput,
            (QueueDirection::Capture, QueueClass::VideoOverlay) => Self::VideoOverlay,
            (QueueDirection::Output, QueueClass::VideoOverlay) => Self::VideoOutputOverlay,
            (QueueDirection::Capture, QueueClass::Vbi) => Self::VbiCapture,
            (QueueDirection::Output, QueueClass::Vbi) => Self::VbiOutput,
            (QueueDirection::Capture, QueueClass::SlicedVbi) => Self::SlicedVbiCapture,
            (QueueDirection::Output, QueueClass::SlicedVbi) => Self::SlicedVbiOutput,
            (QueueDirection::Capture, QueueClass::VideoMplane) => Self::VideoCaptureMplane,
            (QueueDirection::Output, QueueClass::VideoMplane) => Self::VideoOutputMplane,
            (QueueDirection::Capture, QueueClass::Sdr) => Self::SdrCapture,
            (QueueDirection::Output, QueueClass::Sdr) => Self::SdrOutput,
            (QueueDirection::Capture, QueueClass::Meta) => Self::MetaCapture,
            (QueueDirection::Output, QueueClass::Meta) => Self::MetaOutput,
        }
    }

    /// Returns whether the queue type is multiplanar.
    pub fn is_multiplanar(&self) -> bool {
        matches!(
            self,
            QueueType::VideoCaptureMplane | QueueType::VideoOutputMplane
        )
    }

    /// Returns the direction of the queue type (Output or Capture).
    pub fn direction(&self) -> QueueDirection {
        match self {
            QueueType::VideoOutput
            | QueueType::VideoOutputMplane
            | QueueType::VideoOverlay
            | QueueType::VideoOutputOverlay
            | QueueType::VbiOutput
            | QueueType::SlicedVbiOutput
            | QueueType::SdrOutput
            | QueueType::MetaOutput => QueueDirection::Output,

            QueueType::VideoCapture
            | QueueType::VbiCapture
            | QueueType::SlicedVbiCapture
            | QueueType::VideoCaptureMplane
            | QueueType::SdrCapture
            | QueueType::MetaCapture => QueueDirection::Capture,
        }
    }

    pub fn class(&self) -> QueueClass {
        match self {
            QueueType::VideoCapture | QueueType::VideoOutput => QueueClass::Video,
            QueueType::VideoOverlay | QueueType::VideoOutputOverlay => QueueClass::VideoOverlay,
            QueueType::VbiCapture | QueueType::VbiOutput => QueueClass::Vbi,
            QueueType::SlicedVbiCapture | QueueType::SlicedVbiOutput => QueueClass::SlicedVbi,
            QueueType::VideoCaptureMplane | QueueType::VideoOutputMplane => QueueClass::VideoMplane,
            QueueType::SdrCapture | QueueType::SdrOutput => QueueClass::Sdr,
            QueueType::MetaCapture | QueueType::MetaOutput => QueueClass::Meta,
        }
    }
}

impl Display for QueueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

/// A Fourcc pixel format, used to pass formats to V4L2. It can be converted
/// back and forth from a 32-bit integer, or a 4-bytes string.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct PixelFormat(u32);

impl PixelFormat {
    pub const fn from_u32(v: u32) -> Self {
        Self(v)
    }

    pub const fn to_u32(self) -> u32 {
        self.0
    }

    pub const fn from_fourcc(n: &[u8; 4]) -> Self {
        Self(n[0] as u32 | (n[1] as u32) << 8 | (n[2] as u32) << 16 | (n[3] as u32) << 24)
    }

    pub const fn to_fourcc(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
}

/// Converts a Fourcc in 32-bit integer format (like the ones passed in V4L2
/// structures) into the matching pixel format.
///
/// # Examples
///
/// ```
/// # use v4l2r::PixelFormat;
/// // Fourcc representation of NV12.
/// let nv12 = u32::from_le(0x3231564e);
/// let f = PixelFormat::from(nv12);
/// assert_eq!(u32::from(f), nv12);
/// ```
impl From<u32> for PixelFormat {
    fn from(i: u32) -> Self {
        Self::from_u32(i)
    }
}

/// Converts a pixel format back to its 32-bit representation.
///
/// # Examples
///
/// ```
/// # use v4l2r::PixelFormat;
/// // Fourcc representation of NV12.
/// let nv12 = u32::from_le(0x3231564e);
/// let f = PixelFormat::from(nv12);
/// assert_eq!(u32::from(f), nv12);
/// ```
impl From<PixelFormat> for u32 {
    fn from(format: PixelFormat) -> Self {
        format.to_u32()
    }
}

/// Simple way to convert a string litteral (e.g. b"NV12") into a pixel
/// format that can be passed to V4L2.
///
/// # Examples
///
/// ```
/// # use v4l2r::PixelFormat;
/// let nv12 = b"NV12";
/// let f = PixelFormat::from(nv12);
/// assert_eq!(&<[u8; 4]>::from(f), nv12);
/// ```
impl From<&[u8; 4]> for PixelFormat {
    fn from(n: &[u8; 4]) -> Self {
        Self::from_fourcc(n)
    }
}

/// Convert a pixel format back to its 4-character representation.
///
/// # Examples
///
/// ```
/// # use v4l2r::PixelFormat;
/// let nv12 = b"NV12";
/// let f = PixelFormat::from(nv12);
/// assert_eq!(&<[u8; 4]>::from(f), nv12);
/// ```
impl From<PixelFormat> for [u8; 4] {
    fn from(format: PixelFormat) -> Self {
        format.to_fourcc()
    }
}

/// Produces a debug string for this PixelFormat, including its hexadecimal
/// and string representation.
///
/// # Examples
///
/// ```
/// # use v4l2r::PixelFormat;
/// // Fourcc representation of NV12.
/// let nv12 = u32::from_le(0x3231564e);
/// let f = PixelFormat::from(nv12);
/// assert_eq!(format!("{:?}", f), "0x3231564e (NV12)");
/// ```
impl fmt::Debug for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("0x{:08x} ({})", self.0, self))
    }
}

/// Produces a displayable form of this PixelFormat.
///
/// # Examples
///
/// ```
/// # use v4l2r::PixelFormat;
/// // Fourcc representation of NV12.
/// let nv12 = u32::from_le(0x3231564e);
/// let f = PixelFormat::from(nv12);
/// assert_eq!(f.to_string(), "NV12");
/// ```
impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let fourcc = self
            .0
            .to_le_bytes()
            .iter()
            .map(|&x| x as char)
            .collect::<String>();
        f.write_str(fourcc.as_str())
    }
}

/// Description of a single plane in a format.
#[derive(Debug, PartialEq, Clone, Default)]
pub struct PlaneLayout {
    /// Useful size of the plane ; the backing memory must be at least that large.
    pub sizeimage: u32,
    /// Bytes per line of data. Only meaningful for image formats.
    pub bytesperline: u32,
}

/// Unified representation of a V4L2 format capable of handling both single
/// and multi-planar formats. When the single-planar API is used, only
/// one plane shall be used - attempts to have more will be rejected by the
/// ioctl wrappers.
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Format {
    /// Width of the image in pixels.
    pub width: u32,
    /// Height of the image in pixels.
    pub height: u32,
    /// Format each pixel is encoded in.
    pub pixelformat: PixelFormat,
    /// Individual layout of each plane in this format. The exact number of planes
    /// is defined by `pixelformat`.
    pub plane_fmt: Vec<PlaneLayout>,
}

#[derive(Debug, Error, PartialEq)]
pub enum FormatConversionError {
    #[error("too many planes ({0}) specified,")]
    TooManyPlanes(usize),
    #[error("invalid buffer type requested")]
    InvalidBufferType(u32),
}

impl TryFrom<bindings::v4l2_format> for Format {
    type Error = FormatConversionError;

    fn try_from(fmt: bindings::v4l2_format) -> std::result::Result<Self, Self::Error> {
        match fmt.type_ {
            bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE
            | bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_OUTPUT => {
                let pix = unsafe { &fmt.fmt.pix };
                Ok(Format {
                    width: pix.width,
                    height: pix.height,
                    pixelformat: PixelFormat::from(pix.pixelformat),
                    plane_fmt: vec![PlaneLayout {
                        bytesperline: pix.bytesperline,
                        sizeimage: pix.sizeimage,
                    }],
                })
            }
            bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE
            | bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_OUTPUT_MPLANE => {
                let pix_mp = unsafe { &fmt.fmt.pix_mp };

                // Can only happen if we passed a malformed v4l2_format.
                if pix_mp.num_planes as usize > pix_mp.plane_fmt.len() {
                    return Err(Self::Error::TooManyPlanes(pix_mp.num_planes as usize));
                }

                let mut plane_fmt = Vec::new();
                for i in 0..pix_mp.num_planes as usize {
                    let plane = &pix_mp.plane_fmt[i];
                    plane_fmt.push(PlaneLayout {
                        sizeimage: plane.sizeimage,
                        bytesperline: plane.bytesperline,
                    });
                }

                Ok(Format {
                    width: pix_mp.width,
                    height: pix_mp.height,
                    pixelformat: PixelFormat::from(pix_mp.pixelformat),
                    plane_fmt,
                })
            }
            t => Err(Self::Error::InvalidBufferType(t)),
        }
    }
}

/// Quickly build a usable `Format` from a pixel format and resolution.
///
/// # Examples
///
/// ```
/// # use v4l2r::Format;
/// let f = Format::from((b"NV12", (640, 480)));
/// assert_eq!(f.width, 640);
/// assert_eq!(f.height, 480);
/// assert_eq!(f.pixelformat.to_string(), "NV12");
/// assert_eq!(f.plane_fmt.len(), 0);
/// ```
impl<T: Into<PixelFormat>> From<(T, (usize, usize))> for Format {
    fn from((pixel_format, (width, height)): (T, (usize, usize))) -> Self {
        Format {
            width: width as u32,
            height: height as u32,
            pixelformat: pixel_format.into(),
            ..Default::default()
        }
    }
}

/// A more elegant representation for `v4l2_rect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(left: i32, top: i32, width: u32, height: u32) -> Rect {
        Rect {
            left,
            top,
            width,
            height,
        }
    }
}

impl From<bindings::v4l2_rect> for Rect {
    fn from(rect: bindings::v4l2_rect) -> Self {
        Rect {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
        }
    }
}

impl From<bindings::v4l2_selection> for Rect {
    fn from(selection: bindings::v4l2_selection) -> Self {
        Self::from(selection.r)
    }
}

impl From<Rect> for bindings::v4l2_rect {
    fn from(rect: Rect) -> Self {
        bindings::v4l2_rect {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
        }
    }
}

impl Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({}, {}), {}x{}",
            self.left, self.top, self.width, self.height
        )
    }
}

/// Equivalent of `enum v4l2_colorspace`.
#[repr(u32)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, N)]
pub enum Colorspace {
    #[default]
    Default = bindings::v4l2_colorspace_V4L2_COLORSPACE_DEFAULT,
    Smpte170M = bindings::v4l2_colorspace_V4L2_COLORSPACE_SMPTE170M,
    Smpte240M = bindings::v4l2_colorspace_V4L2_COLORSPACE_SMPTE240M,
    Rec709 = bindings::v4l2_colorspace_V4L2_COLORSPACE_REC709,
    Bt878 = bindings::v4l2_colorspace_V4L2_COLORSPACE_BT878,
    SystemM470 = bindings::v4l2_colorspace_V4L2_COLORSPACE_470_SYSTEM_M,
    SystemBG470 = bindings::v4l2_colorspace_V4L2_COLORSPACE_470_SYSTEM_BG,
    Jpeg = bindings::v4l2_colorspace_V4L2_COLORSPACE_JPEG,
    Srgb = bindings::v4l2_colorspace_V4L2_COLORSPACE_SRGB,
    OpRgb = bindings::v4l2_colorspace_V4L2_COLORSPACE_OPRGB,
    Bt2020 = bindings::v4l2_colorspace_V4L2_COLORSPACE_BT2020,
    Raw = bindings::v4l2_colorspace_V4L2_COLORSPACE_RAW,
    DciP3 = bindings::v4l2_colorspace_V4L2_COLORSPACE_DCI_P3,
}

/// Equivalent of `enum v4l2_xfer_func`.
#[repr(u32)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, N)]
pub enum XferFunc {
    #[default]
    Default = bindings::v4l2_xfer_func_V4L2_XFER_FUNC_DEFAULT,
    F709 = bindings::v4l2_xfer_func_V4L2_XFER_FUNC_709,
    Srgb = bindings::v4l2_xfer_func_V4L2_XFER_FUNC_SRGB,
    OpRgb = bindings::v4l2_xfer_func_V4L2_XFER_FUNC_OPRGB,
    Smpte240M = bindings::v4l2_xfer_func_V4L2_XFER_FUNC_SMPTE240M,
    None = bindings::v4l2_xfer_func_V4L2_XFER_FUNC_NONE,
    DciP3 = bindings::v4l2_xfer_func_V4L2_XFER_FUNC_DCI_P3,
    Smpte2084 = bindings::v4l2_xfer_func_V4L2_XFER_FUNC_SMPTE2084,
}

/// Equivalent of `enum v4l2_ycbcr_encoding`.
#[repr(u32)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, N)]
pub enum YCbCrEncoding {
    #[default]
    Default = bindings::v4l2_ycbcr_encoding_V4L2_YCBCR_ENC_DEFAULT,
    E601 = bindings::v4l2_ycbcr_encoding_V4L2_YCBCR_ENC_601,
    E709 = bindings::v4l2_ycbcr_encoding_V4L2_YCBCR_ENC_709,
    Xv601 = bindings::v4l2_ycbcr_encoding_V4L2_YCBCR_ENC_XV601,
    Xv709 = bindings::v4l2_ycbcr_encoding_V4L2_YCBCR_ENC_XV709,
    Sycc = bindings::v4l2_ycbcr_encoding_V4L2_YCBCR_ENC_SYCC,
    Bt2020 = bindings::v4l2_ycbcr_encoding_V4L2_YCBCR_ENC_BT2020,
    Bt2020ConstLum = bindings::v4l2_ycbcr_encoding_V4L2_YCBCR_ENC_BT2020_CONST_LUM,
    Smpte240M = bindings::v4l2_ycbcr_encoding_V4L2_YCBCR_ENC_SMPTE240M,
}

/// Equivalent of `enum v4l2_quantization`.
#[repr(u32)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, N)]
pub enum Quantization {
    #[default]
    Default = bindings::v4l2_quantization_V4L2_QUANTIZATION_DEFAULT,
    FullRange = bindings::v4l2_quantization_V4L2_QUANTIZATION_FULL_RANGE,
    LimRange = bindings::v4l2_quantization_V4L2_QUANTIZATION_LIM_RANGE,
}
