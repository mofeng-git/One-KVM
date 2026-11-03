//! Packed 10-bit HDMI RX layouts accepted by the HDR tone-mapping stage.
//!
//! `snps_hdmirx` (driver patch "10bit-hdr-v3") stores 4:2:2 as `NV20` and
//! 4:2:0 as `NV15`: four 10-bit samples packed little-endian into five bytes,
//! rows padded to 64 bytes. The compute shader is specialised for 3840x2160.

pub const WIDTH: u32 = 3840;
pub const HEIGHT: u32 = 2160;
/// Unpadded packed row: 3840 * 10 / 8.
pub const MIN_STRIDE: u32 = WIDTH / 4 * 5;
/// Largest row pitch the shader's `uint` addressing is validated for.
pub const MAX_STRIDE: u32 = 4864;
/// Size of the NV12 frame produced by the tone mapper (stride == WIDTH).
pub const NV12_BYTES: usize = (WIDTH as usize) * (HEIGHT as usize) * 3 / 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackedFormat {
    /// 4:2:2, full-height chroma plane.
    Nv20,
    /// 4:2:0, half-height chroma plane.
    Nv15,
}

impl PackedFormat {
    pub fn from_fourcc(fourcc: [u8; 4]) -> Option<Self> {
        match &fourcc {
            b"NV20" => Some(Self::Nv20),
            b"NV15" => Some(Self::Nv15),
            _ => None,
        }
    }

    pub fn is_422(self) -> bool {
        self == Self::Nv20
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedLayout {
    pub format: PackedFormat,
    pub stride: u32,
}

impl PackedLayout {
    /// `None` unless the geometry is exactly what the shader was built for.
    pub fn new(format: PackedFormat, width: u32, height: u32, stride: u32) -> Option<Self> {
        let valid = width == WIDTH
            && height == HEIGHT
            && (MIN_STRIDE..=MAX_STRIDE).contains(&stride)
            && stride % 4 == 0;
        valid.then_some(Self { format, stride })
    }

    /// Bytes in the luma plane plus the chroma plane (== V4L2 `sizeimage`).
    pub fn bytes(&self) -> usize {
        // Luma plane plus a full-height (4:2:2) or half-height (4:2:0) chroma plane.
        let half_rows = if self.format.is_422() { 4 } else { 3 };
        self.stride as usize * HEIGHT as usize * half_rows / 2
    }

    /// Offset of the chroma plane, in bytes.
    pub fn chroma_offset(&self) -> u32 {
        self.stride * HEIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nv20_and_nv15_sizes_match_the_driver() {
        let nv20 = PackedLayout::new(PackedFormat::Nv20, 3840, 2160, 4800).unwrap();
        assert_eq!(nv20.bytes(), 4800 * 2160 * 2);
        let nv15 = PackedLayout::new(PackedFormat::Nv15, 3840, 2160, 4800).unwrap();
        assert_eq!(nv15.bytes(), 4800 * 2160 * 3 / 2);
        assert_eq!(nv20.chroma_offset(), 4800 * 2160);
        assert_eq!(NV12_BYTES, 12_441_600);
    }

    #[test]
    fn padded_strides_are_accepted_and_others_rejected() {
        assert!(PackedLayout::new(PackedFormat::Nv20, 3840, 2160, 4864).is_some());
        for (w, h, stride) in [
            (3840, 2160, 4799),
            (3840, 2160, 4865),
            (3840, 2160, 4802),
            (1920, 1080, 2400),
            (3840, 2161, 4800),
            (4096, 2160, 4800),
            (3840, 2160, 0),
            (u32::MAX, u32::MAX, u32::MAX),
        ] {
            assert!(PackedLayout::new(PackedFormat::Nv20, w, h, stride).is_none());
        }
    }

    #[test]
    fn only_the_packed_ten_bit_fourccs_are_recognised() {
        assert_eq!(
            PackedFormat::from_fourcc(*b"NV20"),
            Some(PackedFormat::Nv20)
        );
        assert_eq!(
            PackedFormat::from_fourcc(*b"NV15"),
            Some(PackedFormat::Nv15)
        );
        for fourcc in [*b"NV12", *b"NV16", *b"P010", *b"BGR3"] {
            assert_eq!(PackedFormat::from_fourcc(fourcc), None);
        }
    }
}
