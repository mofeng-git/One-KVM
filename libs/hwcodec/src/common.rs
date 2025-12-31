#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use serde_derive::{Deserialize, Serialize};
include!(concat!(env!("OUT_DIR"), "/common_ffi.rs"));

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Driver {
    NV,
    AMF,
    MFX,
    FFMPEG,
}

#[cfg(any(windows, target_os = "linux"))]
pub(crate) fn supported_gpu(_encode: bool) -> (bool, bool, bool) {
    #[cfg(target_os = "linux")]
    use std::ffi::c_int;
    #[cfg(target_os = "linux")]
    extern "C" {
        pub(crate) fn linux_support_nv() -> c_int;
        pub(crate) fn linux_support_amd() -> c_int;
        pub(crate) fn linux_support_intel() -> c_int;
    }

    #[allow(unused_unsafe)]
    unsafe {
        #[cfg(windows)]
        {
            // Without VRAM feature, assume all GPU types might be available
            // FFmpeg will handle the actual detection
            return (true, true, true);
        }

        #[cfg(target_os = "linux")]
        return (
            linux_support_nv() == 0,
            linux_support_amd() == 0,
            linux_support_intel() == 0,
        );
        #[allow(unreachable_code)]
        (false, false, false)
    }
}

pub fn get_gpu_signature() -> u64 {
    #[cfg(windows)]
    {
        extern "C" {
            pub fn GetHwcodecGpuSignature() -> u64;
        }
        unsafe { GetHwcodecGpuSignature() }
    }
    #[cfg(not(windows))]
    {
        0
    }
}

#[cfg(target_os = "linux")]
pub fn setup_parent_death_signal() {
    use std::sync::Once;

    static INIT: Once = Once::new();

    INIT.call_once(|| {
        use std::ffi::c_int;
        extern "C" {
            fn setup_parent_death_signal() -> c_int;
        }
        unsafe {
            let result = setup_parent_death_signal();
            if result == 0 {
                log::debug!("Successfully set up parent death signal");
            } else {
                log::warn!("Failed to set up parent death signal: {}", result);
            }
        }
    });
}

#[cfg(windows)]
pub fn child_exit_when_parent_exit(child_process_id: u32) -> bool {
    unsafe {
        extern "C" {
             fn add_process_to_new_job(child_process_id: u32) -> i32;
        }
        let result = add_process_to_new_job(child_process_id);
        result == 0
    }
}
