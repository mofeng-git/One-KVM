#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused_imports)]

include!(concat!(env!("OUT_DIR"), "/ffmpeg_ffi.rs"));

use serde_derive::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum AVHWDeviceType {
    AV_HWDEVICE_TYPE_NONE,
    AV_HWDEVICE_TYPE_VDPAU,
    AV_HWDEVICE_TYPE_CUDA,
    AV_HWDEVICE_TYPE_VAAPI,
    AV_HWDEVICE_TYPE_DXVA2,
    AV_HWDEVICE_TYPE_QSV,
    AV_HWDEVICE_TYPE_VIDEOTOOLBOX,
    AV_HWDEVICE_TYPE_D3D11VA,
    AV_HWDEVICE_TYPE_DRM,
    AV_HWDEVICE_TYPE_OPENCL,
    AV_HWDEVICE_TYPE_MEDIACODEC,
    AV_HWDEVICE_TYPE_VULKAN,
}

#[no_mangle]
pub extern "C" fn hwcodec_av_log_callback(level: i32, message: *const std::os::raw::c_char) {
    let could_not_find_ref_with_poc = "Could not find ref with POC";
    unsafe {
        let c_str = std::ffi::CStr::from_ptr(message);
        if let Ok(str_slice) = c_str.to_str() {
            let string = String::from(str_slice);
            if level == AV_LOG_ERROR as i32 {
                log::error!("{}", string);
                if string.contains(could_not_find_ref_with_poc) {
                    hwcodec_set_flag_could_not_find_ref_with_poc();
                }
            } else if level == AV_LOG_PANIC as i32 || level == AV_LOG_FATAL as i32 {
                log::error!("{}", string);
            } else if level == AV_LOG_WARNING as i32 {
                log::warn!("{}", string);
            } else if level == AV_LOG_INFO as i32 {
                log::info!("{}", string);
            } else if level == AV_LOG_VERBOSE as i32 || level == AV_LOG_DEBUG as i32 {
                log::debug!("{}", string);
            } else if level == AV_LOG_TRACE as i32 {
                log::trace!("{}", string);
            }
        }
    }
}

pub(crate) fn init_av_log() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        av_log_set_level(parse_ffmpeg_log_level());
        hwcodec_set_av_log_callback();
    });
}

fn parse_ffmpeg_log_level() -> i32 {
    let raw = match env::var("ONE_KVM_FFMPEG_LOG") {
        Ok(value) => value,
        Err(_) => return AV_LOG_ERROR as i32,
    };

    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty() {
        return AV_LOG_ERROR as i32;
    }

    if let Ok(level) = value.parse::<i32>() {
        return level;
    }

    match value.as_str() {
        "quiet" => AV_LOG_QUIET as i32,
        "panic" => AV_LOG_PANIC as i32,
        "fatal" => AV_LOG_FATAL as i32,
        "error" => AV_LOG_ERROR as i32,
        "warn" | "warning" => AV_LOG_WARNING as i32,
        "info" => AV_LOG_INFO as i32,
        "verbose" => AV_LOG_VERBOSE as i32,
        "debug" => AV_LOG_DEBUG as i32,
        "trace" => AV_LOG_TRACE as i32,
        _ => AV_LOG_ERROR as i32,
    }
}
