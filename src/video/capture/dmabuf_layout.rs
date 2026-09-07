//! Conservative, dependency-free eligibility checks for linear RKMPP input.

pub struct DmaCaptureLayout<'a> {
    pub native_hdmi: bool,
    pub driver: &'a str,
    pub bus_info: &'a str,
    pub configurable_usb: bool,
    pub single_planar: bool,
    pub fourcc: [u8; 4],
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

impl DmaCaptureLayout<'_> {
    /// Minimum readable bytes, not the driver's page-aligned allocation size.
    pub fn minimum_bytes(&self) -> Option<usize> {
        if self.width == 0
            || self.height == 0
            || self.width > 8192
            || self.height > 8192
            || self.width % 2 != 0
            || self.height % 2 != 0
        {
            return None;
        }
        let bytes_per_row = if self.native_hdmi {
            match &self.fourcc {
                b"NV12" => self.width,
                b"BGR3" => self.width.checked_mul(3)?,
                _ => return None,
            }
        } else if self.configurable_usb
            && self.single_planar
            && self.driver == "uvcvideo"
            && self.bus_info.starts_with("usb-")
        {
            match &self.fourcc {
                // Compressed frames have variable bytesused and no byte stride.
                b"MJPG" => return Some(4),
                b"YUYV" if self.stride % 16 == 0 => self.width.checked_mul(2)?,
                b"NV12" if self.stride % 16 == 0 => self.width,
                b"RGB3" if self.stride % 16 == 0 => self.width.checked_mul(3)?,
                _ => return None,
            }
        } else {
            return None;
        };
        if self.stride < bytes_per_row {
            return None;
        }
        let size = (self.stride as usize).checked_mul(self.height as usize)?;
        if self.fourcc == *b"NV12" {
            size.checked_mul(3)?.checked_div(2)
        } else {
            Some(size)
        }
    }
}

/// An MJPEG DMA packet has a bounded payload, not stride * height bytes.
/// Reserve readable headroom for the MPP bitstream reader without modifying
/// capture memory. The decoder receives only `used`, never the allocation size.
pub fn valid_payload(
    compressed: bool,
    used: usize,
    capacity: usize,
    expected: Option<usize>,
) -> bool {
    if compressed {
        used >= 4 && used.checked_add(64).is_some_and(|end| end <= capacity)
    } else {
        used > 0 && Some(used) == expected && used <= capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb() -> DmaCaptureLayout<'static> {
        DmaCaptureLayout {
            native_hdmi: false,
            driver: "uvcvideo",
            bus_info: "usb-fc880000.usb-1.1",
            configurable_usb: true,
            single_planar: true,
            fourcc: *b"YUYV",
            width: 1920,
            height: 1080,
            stride: 3840,
        }
    }

    #[test]
    fn usb_yuyv_uses_byte_stride_and_supports_padding() {
        let mut layout = usb();
        assert_eq!(layout.minimum_bytes(), Some(4_147_200));
        layout.width = 640;
        layout.height = 480;
        layout.stride = 1280;
        assert_eq!(layout.minimum_bytes(), Some(614_400));
        layout.stride = 1296;
        assert_eq!(layout.minimum_bytes(), Some(622_080));
    }

    #[test]
    fn usb_requires_correct_driver_bus_queue_and_control_mode() {
        let mut layout = usb();
        layout.driver = "rkcif";
        assert_eq!(layout.minimum_bytes(), None);
        layout = usb();
        layout.bus_info = "platform:hdmi";
        assert_eq!(layout.minimum_bytes(), None);
        layout = usb();
        layout.single_planar = false;
        assert_eq!(layout.minimum_bytes(), None);
        layout = usb();
        layout.configurable_usb = false;
        assert_eq!(layout.minimum_bytes(), None);
    }

    #[test]
    fn unverified_usb_formats_stay_on_copy_path() {
        for fourcc in [
            *b"H264", *b"NV21", *b"NV16", *b"NV24", *b"BGR3", *b"YU12", *b"UYVY", *b"YVYU",
            *b"BAD!",
        ] {
            let mut layout = usb();
            layout.fourcc = fourcc;
            assert_eq!(layout.minimum_bytes(), None, "{fourcc:?}");
        }
    }

    #[test]
    fn usb_nv12_rgb_and_mjpeg_layouts() {
        let mut layout = usb();
        layout.fourcc = *b"NV12";
        layout.stride = 1920;
        assert_eq!(layout.minimum_bytes(), Some(3_110_400));
        layout.fourcc = *b"RGB3";
        layout.stride = 5760;
        assert_eq!(layout.minimum_bytes(), Some(6_220_800));
        layout.stride = 1920;
        assert_eq!(layout.minimum_bytes(), None);
        layout.fourcc = *b"MJPG";
        layout.stride = 0;
        assert_eq!(layout.minimum_bytes(), Some(4));
    }

    #[test]
    fn compressed_payload_is_bounded_and_not_allocation_size() {
        assert!(valid_payload(true, 63163, 4147200, None));
        for used in [0, 3, 4147200, usize::MAX] {
            assert!(!valid_payload(true, used, 4147200, None));
        }
        assert!(valid_payload(true, 4, 68, None));
        assert!(!valid_payload(true, 4, 67, None));
        assert!(valid_payload(false, 614400, 614400, Some(614400)));
        assert!(!valid_payload(false, 614399, 614400, Some(614400)));
        assert!(!valid_payload(false, 614400, 614399, Some(614400)));
    }

    #[test]
    fn malformed_geometry_or_stride_is_rejected() {
        for (w, h, stride) in [
            (0, 1080, 3840),
            (1920, 0, 3840),
            (1919, 1080, 3840),
            (1920, 1079, 3840),
            (8194, 1080, 16384),
            (1920, 8194, 3840),
            (1920, 1080, 0),
            (1920, 1080, 1920),
            (1920, 1080, 3841),
            (u32::MAX, u32::MAX, u32::MAX),
        ] {
            let mut layout = usb();
            layout.width = w;
            layout.height = h;
            layout.stride = stride;
            assert_eq!(layout.minimum_bytes(), None);
        }
    }

    #[test]
    fn native_hdmi_formats_are_preserved_but_not_expanded() {
        let mut layout = usb();
        layout.native_hdmi = true;
        layout.single_planar = false;
        layout.fourcc = *b"BGR3";
        layout.stride = 5760;
        assert_eq!(layout.minimum_bytes(), Some(6_220_800));
        layout.fourcc = *b"NV12";
        layout.stride = 1920;
        assert_eq!(layout.minimum_bytes(), Some(3_110_400));
        layout.fourcc = *b"YUYV";
        layout.stride = 3840;
        assert_eq!(layout.minimum_bytes(), None);
    }
}
