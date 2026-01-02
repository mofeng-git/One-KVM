//! Pixel format definitions and conversions

use serde::{Deserialize, Serialize};
use std::fmt;
use v4l::format::fourcc;

/// Supported pixel formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PixelFormat {
    /// MJPEG compressed format (preferred for capture cards)
    Mjpeg,
    /// JPEG compressed format
    Jpeg,
    /// YUYV 4:2:2 packed format
    Yuyv,
    /// YVYU 4:2:2 packed format
    Yvyu,
    /// UYVY 4:2:2 packed format
    Uyvy,
    /// NV12 semi-planar format (Y plane + interleaved UV)
    Nv12,
    /// NV16 semi-planar format
    Nv16,
    /// NV24 semi-planar format
    Nv24,
    /// YUV420 planar format
    Yuv420,
    /// YVU420 planar format
    Yvu420,
    /// RGB565 format
    Rgb565,
    /// RGB24 format (3 bytes per pixel)
    Rgb24,
    /// BGR24 format (3 bytes per pixel)
    Bgr24,
    /// Grayscale format
    Grey,
}

impl PixelFormat {
    /// Convert to V4L2 FourCC
    pub fn to_fourcc(&self) -> fourcc::FourCC {
        match self {
            PixelFormat::Mjpeg => fourcc::FourCC::new(b"MJPG"),
            PixelFormat::Jpeg => fourcc::FourCC::new(b"JPEG"),
            PixelFormat::Yuyv => fourcc::FourCC::new(b"YUYV"),
            PixelFormat::Yvyu => fourcc::FourCC::new(b"YVYU"),
            PixelFormat::Uyvy => fourcc::FourCC::new(b"UYVY"),
            PixelFormat::Nv12 => fourcc::FourCC::new(b"NV12"),
            PixelFormat::Nv16 => fourcc::FourCC::new(b"NV16"),
            PixelFormat::Nv24 => fourcc::FourCC::new(b"NV24"),
            PixelFormat::Yuv420 => fourcc::FourCC::new(b"YU12"),
            PixelFormat::Yvu420 => fourcc::FourCC::new(b"YV12"),
            PixelFormat::Rgb565 => fourcc::FourCC::new(b"RGBP"),
            PixelFormat::Rgb24 => fourcc::FourCC::new(b"RGB3"),
            PixelFormat::Bgr24 => fourcc::FourCC::new(b"BGR3"),
            PixelFormat::Grey => fourcc::FourCC::new(b"GREY"),
        }
    }

    /// Try to convert from V4L2 FourCC
    pub fn from_fourcc(fourcc: fourcc::FourCC) -> Option<Self> {
        let repr = fourcc.repr;
        match &repr {
            b"MJPG" => Some(PixelFormat::Mjpeg),
            b"JPEG" => Some(PixelFormat::Jpeg),
            b"YUYV" => Some(PixelFormat::Yuyv),
            b"YVYU" => Some(PixelFormat::Yvyu),
            b"UYVY" => Some(PixelFormat::Uyvy),
            b"NV12" => Some(PixelFormat::Nv12),
            b"NV16" => Some(PixelFormat::Nv16),
            b"NV24" => Some(PixelFormat::Nv24),
            b"YU12" | b"I420" => Some(PixelFormat::Yuv420),
            b"YV12" => Some(PixelFormat::Yvu420),
            b"RGBP" => Some(PixelFormat::Rgb565),
            b"RGB3" => Some(PixelFormat::Rgb24),
            b"BGR3" => Some(PixelFormat::Bgr24),
            b"GREY" | b"Y800" => Some(PixelFormat::Grey),
            _ => None,
        }
    }

    /// Check if format is compressed (JPEG/MJPEG)
    pub fn is_compressed(&self) -> bool {
        matches!(self, PixelFormat::Mjpeg | PixelFormat::Jpeg)
    }

    /// Get bytes per pixel for uncompressed formats
    /// Returns None for compressed formats
    pub fn bytes_per_pixel(&self) -> Option<usize> {
        match self {
            PixelFormat::Mjpeg | PixelFormat::Jpeg => None,
            PixelFormat::Yuyv | PixelFormat::Yvyu | PixelFormat::Uyvy => Some(2),
            PixelFormat::Nv12 | PixelFormat::Yuv420 | PixelFormat::Yvu420 => None, // Variable
            PixelFormat::Nv16 => None,
            PixelFormat::Nv24 => None,
            PixelFormat::Rgb565 => Some(2),
            PixelFormat::Rgb24 | PixelFormat::Bgr24 => Some(3),
            PixelFormat::Grey => Some(1),
        }
    }

    /// Calculate expected frame size for a given resolution
    /// Returns None for compressed formats (variable size)
    pub fn frame_size(&self, resolution: Resolution) -> Option<usize> {
        let pixels = (resolution.width * resolution.height) as usize;
        match self {
            PixelFormat::Mjpeg | PixelFormat::Jpeg => None,
            PixelFormat::Yuyv | PixelFormat::Yvyu | PixelFormat::Uyvy => Some(pixels * 2),
            PixelFormat::Nv12 | PixelFormat::Yuv420 | PixelFormat::Yvu420 => Some(pixels * 3 / 2),
            PixelFormat::Nv16 => Some(pixels * 2),
            PixelFormat::Nv24 => Some(pixels * 3),
            PixelFormat::Rgb565 => Some(pixels * 2),
            PixelFormat::Rgb24 | PixelFormat::Bgr24 => Some(pixels * 3),
            PixelFormat::Grey => Some(pixels),
        }
    }

    /// Get priority for format selection (higher is better)
    /// MJPEG is preferred for HDMI capture cards
    pub fn priority(&self) -> u8 {
        match self {
            PixelFormat::Mjpeg => 100,
            PixelFormat::Jpeg => 99,
            PixelFormat::Yuyv => 80,
            PixelFormat::Nv12 => 75,
            PixelFormat::Yuv420 => 70,
            PixelFormat::Uyvy => 65,
            PixelFormat::Yvyu => 64,
            PixelFormat::Yvu420 => 63,
            PixelFormat::Nv16 => 60,
            PixelFormat::Nv24 => 55,
            PixelFormat::Rgb24 => 50,
            PixelFormat::Bgr24 => 49,
            PixelFormat::Rgb565 => 40,
            PixelFormat::Grey => 10,
        }
    }

    /// Get recommended format for video encoding (WebRTC)
    ///
    /// Hardware encoding prefers: NV12 > YUYV
    /// Software encoding prefers: YUYV > NV12
    ///
    /// Returns None if no suitable format is available
    pub fn recommended_for_encoding(available: &[PixelFormat], is_hardware: bool) -> Option<PixelFormat> {
        if is_hardware {
            // Hardware encoding: NV12 > YUYV
            if available.contains(&PixelFormat::Nv12) {
                return Some(PixelFormat::Nv12);
            }
            if available.contains(&PixelFormat::Yuyv) {
                return Some(PixelFormat::Yuyv);
            }
        } else {
            // Software encoding: YUYV > NV12
            if available.contains(&PixelFormat::Yuyv) {
                return Some(PixelFormat::Yuyv);
            }
            if available.contains(&PixelFormat::Nv12) {
                return Some(PixelFormat::Nv12);
            }
        }
        // Fallback to any non-compressed format
        available.iter().find(|f| !f.is_compressed()).copied()
    }

    /// Get all supported formats
    pub fn all() -> &'static [PixelFormat] {
        &[
            PixelFormat::Mjpeg,
            PixelFormat::Jpeg,
            PixelFormat::Yuyv,
            PixelFormat::Yvyu,
            PixelFormat::Uyvy,
            PixelFormat::Nv12,
            PixelFormat::Nv16,
            PixelFormat::Nv24,
            PixelFormat::Yuv420,
            PixelFormat::Yvu420,
            PixelFormat::Rgb565,
            PixelFormat::Rgb24,
            PixelFormat::Bgr24,
            PixelFormat::Grey,
        ]
    }
}

impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            PixelFormat::Mjpeg => "MJPEG",
            PixelFormat::Jpeg => "JPEG",
            PixelFormat::Yuyv => "YUYV",
            PixelFormat::Yvyu => "YVYU",
            PixelFormat::Uyvy => "UYVY",
            PixelFormat::Nv12 => "NV12",
            PixelFormat::Nv16 => "NV16",
            PixelFormat::Nv24 => "NV24",
            PixelFormat::Yuv420 => "YUV420",
            PixelFormat::Yvu420 => "YVU420",
            PixelFormat::Rgb565 => "RGB565",
            PixelFormat::Rgb24 => "RGB24",
            PixelFormat::Bgr24 => "BGR24",
            PixelFormat::Grey => "GREY",
        };
        write!(f, "{}", name)
    }
}

impl std::str::FromStr for PixelFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "MJPEG" | "MJPG" => Ok(PixelFormat::Mjpeg),
            "JPEG" => Ok(PixelFormat::Jpeg),
            "YUYV" => Ok(PixelFormat::Yuyv),
            "YVYU" => Ok(PixelFormat::Yvyu),
            "UYVY" => Ok(PixelFormat::Uyvy),
            "NV12" => Ok(PixelFormat::Nv12),
            "NV16" => Ok(PixelFormat::Nv16),
            "NV24" => Ok(PixelFormat::Nv24),
            "YUV420" | "I420" => Ok(PixelFormat::Yuv420),
            "YVU420" | "YV12" => Ok(PixelFormat::Yvu420),
            "RGB565" => Ok(PixelFormat::Rgb565),
            "RGB24" => Ok(PixelFormat::Rgb24),
            "BGR24" => Ok(PixelFormat::Bgr24),
            "GREY" | "GRAY" => Ok(PixelFormat::Grey),
            _ => Err(format!("Unknown pixel format: {}", s)),
        }
    }
}

/// Resolution (width x height)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Check if resolution is valid
    pub fn is_valid(&self) -> bool {
        self.width >= 160 && self.width <= 15360 && self.height >= 120 && self.height <= 8640
    }

    /// Get total pixels
    pub fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Common resolutions
    pub const VGA: Resolution = Resolution {
        width: 640,
        height: 480,
    };
    pub const HD720: Resolution = Resolution {
        width: 1280,
        height: 720,
    };
    pub const HD1080: Resolution = Resolution {
        width: 1920,
        height: 1080,
    };
    pub const UHD4K: Resolution = Resolution {
        width: 3840,
        height: 2160,
    };
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl From<(u32, u32)> for Resolution {
    fn from((width, height): (u32, u32)) -> Self {
        Self { width, height }
    }
}
