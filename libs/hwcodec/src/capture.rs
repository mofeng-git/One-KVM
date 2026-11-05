#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::ffi::{CStr, CString};
use std::os::raw::c_int;

include!(concat!(env!("OUT_DIR"), "/ffmpeg_capture_ffi.rs"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturePixelFormat {
    Unknown,
    Mjpeg,
    Jpeg,
    Yuyv,
    Yvyu,
    Uyvy,
    Nv12,
    Nv21,
    Nv16,
    Nv24,
    Yuv420,
    Yvu420,
    Rgb24,
    Bgr24,
    Grey,
}

impl CapturePixelFormat {
    pub fn to_ffi(self) -> c_int {
        match self {
            Self::Unknown => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_UNKNOWN as c_int,
            Self::Mjpeg => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_MJPEG as c_int,
            Self::Jpeg => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_JPEG as c_int,
            Self::Yuyv => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_YUYV as c_int,
            Self::Yvyu => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_YVYU as c_int,
            Self::Uyvy => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_UYVY as c_int,
            Self::Nv12 => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_NV12 as c_int,
            Self::Nv21 => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_NV21 as c_int,
            Self::Nv16 => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_NV16 as c_int,
            Self::Nv24 => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_NV24 as c_int,
            Self::Yuv420 => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_YUV420 as c_int,
            Self::Yvu420 => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_YVU420 as c_int,
            Self::Rgb24 => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_RGB24 as c_int,
            Self::Bgr24 => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_BGR24 as c_int,
            Self::Grey => HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_GREY as c_int,
        }
    }

    pub fn from_ffi(value: c_int) -> Self {
        match value {
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_MJPEG as c_int => Self::Mjpeg,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_JPEG as c_int => Self::Jpeg,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_YUYV as c_int => Self::Yuyv,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_YVYU as c_int => Self::Yvyu,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_UYVY as c_int => Self::Uyvy,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_NV12 as c_int => Self::Nv12,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_NV21 as c_int => Self::Nv21,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_NV16 as c_int => Self::Nv16,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_NV24 as c_int => Self::Nv24,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_YUV420 as c_int => {
                Self::Yuv420
            }
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_YVU420 as c_int => {
                Self::Yvu420
            }
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_RGB24 as c_int => Self::Rgb24,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_BGR24 as c_int => Self::Bgr24,
            x if x == HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_GREY as c_int => Self::Grey,
            _ => Self::Unknown,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "MJPEG" | "MJPG" => Some(Self::Mjpeg),
            "JPEG" => Some(Self::Jpeg),
            "YUYV" => Some(Self::Yuyv),
            "YVYU" => Some(Self::Yvyu),
            "UYVY" => Some(Self::Uyvy),
            "NV12" => Some(Self::Nv12),
            "NV21" => Some(Self::Nv21),
            "NV16" => Some(Self::Nv16),
            "NV24" => Some(Self::Nv24),
            "YUV420" | "I420" | "IYUV" => Some(Self::Yuv420),
            "YVU420" | "YV12" => Some(Self::Yvu420),
            "RGB24" => Some(Self::Rgb24),
            "BGR24" => Some(Self::Bgr24),
            "GREY" | "GRAY" | "Y800" => Some(Self::Grey),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DshowCapability {
    pub format: CapturePixelFormat,
    pub width: u32,
    pub height: u32,
    pub fps: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureStreamInfo {
    pub width: i32,
    pub height: i32,
    pub pixel_format: CapturePixelFormat,
    pub stride: i32,
}

#[derive(Debug)]
pub struct CaptureError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CaptureError {}

fn last_error_message() -> String {
    unsafe {
        let ptr = hwcodec_capture_last_error();
        if ptr.is_null() {
            return String::new();
        }
        CStr::from_ptr(ptr).to_string_lossy().to_string()
    }
}

pub fn list_dshow_video_devices() -> Result<Vec<String>, CaptureError> {
    unsafe {
        let ptr = hwcodec_dshow_list_video_devices();
        if ptr.is_null() {
            return Err(CaptureError {
                code: -1,
                message: last_error_message(),
            });
        }
        let payload = CStr::from_ptr(ptr).to_string_lossy().to_string();
        hwcodec_capture_string_free(ptr as *mut _);
        Ok(payload
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    }
}

pub fn list_dshow_device_capabilities(device_name: &str) -> Result<Vec<DshowCapability>, CaptureError> {
    let device_name = CString::new(device_name).map_err(|_| CaptureError {
        code: -1,
        message: "device name contains NUL byte".to_string(),
    })?;

    unsafe {
        let ptr = hwcodec_dshow_list_device_capabilities(device_name.as_ptr());
        if ptr.is_null() {
            return Err(CaptureError {
                code: -1,
                message: last_error_message(),
            });
        }

        let payload = CStr::from_ptr(ptr).to_string_lossy().to_string();
        hwcodec_capture_string_free(ptr as *mut _);

        let capabilities = payload
            .lines()
            .filter_map(parse_dshow_capability_line)
            .collect();
        Ok(capabilities)
    }
}

fn parse_dshow_capability_line(line: &str) -> Option<DshowCapability> {
    let mut parts = line.split('|');
    let format = CapturePixelFormat::from_name(parts.next()?.trim())?;
    let width = parts.next()?.trim().parse::<u32>().ok()?;
    let height = parts.next()?.trim().parse::<u32>().ok()?;
    let fps = parts
        .next()
        .unwrap_or_default()
        .split(',')
        .filter_map(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();

    Some(DshowCapability {
        format,
        width,
        height,
        fps,
    })
}

pub struct DshowCapture {
    ctx: *mut HwcodecDshowCaptureContext,
}

unsafe impl Send for DshowCapture {}

impl DshowCapture {
    pub fn open(
        device_name: &str,
        width: i32,
        height: i32,
        fps: i32,
        requested_format: CapturePixelFormat,
        timeout_ms: i32,
    ) -> Result<Self, CaptureError> {
        let device_name = CString::new(device_name).map_err(|_| CaptureError {
            code: -1,
            message: "device name contains NUL byte".to_string(),
        })?;
        unsafe {
            let ctx = hwcodec_dshow_capture_open(
                device_name.as_ptr(),
                width,
                height,
                fps,
                requested_format.to_ffi(),
                timeout_ms,
            );
            if ctx.is_null() {
                return Err(CaptureError {
                    code: -1,
                    message: last_error_message(),
                });
            }
            Ok(Self { ctx })
        }
    }

    pub fn info(&self) -> Result<CaptureStreamInfo, CaptureError> {
        unsafe {
            let mut info = HwcodecCaptureStreamInfo {
                width: 0,
                height: 0,
                pixel_format: HwcodecCapturePixelFormat::HWCODEC_CAPTURE_FMT_UNKNOWN as c_int,
                stride: 0,
            };
            let ret = hwcodec_dshow_capture_info(self.ctx, &mut info);
            if ret != 0 {
                return Err(CaptureError {
                    code: ret,
                    message: last_error_message(),
                });
            }
            Ok(CaptureStreamInfo {
                width: info.width,
                height: info.height,
                pixel_format: CapturePixelFormat::from_ffi(info.pixel_format),
                stride: info.stride,
            })
        }
    }

    pub fn read_packet(&mut self) -> Result<(Vec<u8>, u64), CaptureError> {
        unsafe {
            let mut data = std::ptr::null_mut();
            let mut len = 0;
            let mut sequence = 0u64;
            let ret = hwcodec_dshow_capture_read(self.ctx, &mut data, &mut len, &mut sequence);
            if ret != 0 {
                return Err(CaptureError {
                    code: ret,
                    message: last_error_message(),
                });
            }
            if data.is_null() || len <= 0 {
                return Err(CaptureError {
                    code: -1,
                    message: "empty packet returned by capture backend".to_string(),
                });
            }
            let slice = std::slice::from_raw_parts(data, len as usize);
            let vec = slice.to_vec();
            hwcodec_dshow_capture_packet_free(data);
            Ok((vec, sequence))
        }
    }
}

impl Drop for DshowCapture {
    fn drop(&mut self) {
        unsafe {
            hwcodec_dshow_capture_close(self.ctx);
        }
        self.ctx = std::ptr::null_mut();
    }
}
