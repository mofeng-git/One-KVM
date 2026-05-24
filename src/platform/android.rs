//! Android Amlogic platform capabilities.

use super::{FeatureCapability, PlatformCapabilities, PlatformMode};

#[cfg(feature = "android")]
#[allow(dead_code)]
fn _keep_android_bionic_ifaddrs_shim_linked() {
    let _ = crate::platform::android_bionic::freeifaddrs
        as unsafe extern "C" fn(*mut crate::platform::android_bionic::ifaddrs);
    let _ = crate::platform::android_bionic::getifaddrs
        as unsafe extern "C" fn(*mut *mut crate::platform::android_bionic::ifaddrs) -> i32;
}

pub fn capabilities() -> PlatformCapabilities {
    #[cfg(feature = "android")]
    _keep_android_bionic_ifaddrs_shim_linked();

    PlatformCapabilities {
        mode: PlatformMode::AndroidAmlogic,
        mode_label: PlatformMode::AndroidAmlogic.label(),
        video_capture: FeatureCapability::available(["v4l2_uvc"])
            .with_selected_backend(Some("v4l2_uvc".to_string())),
        encoder: FeatureCapability::available(["ffmpeg_mediacodec_h264", "mjpeg"])
            .with_selected_backend(Some(
                if cfg!(feature = "android-mediacodec") {
                    "ffmpeg_mediacodec_h264"
                } else {
                    "mjpeg"
                }
                .to_string(),
            )),
        hid: FeatureCapability::available(["otg_configfs", "ch9329", "none"]),
        atx: FeatureCapability::available(["gpio", "usb_relay", "serial", "wol", "none"]),
        msd: FeatureCapability::available(["otg_configfs"]),
        otg: FeatureCapability::available(["configfs"]),
        audio: FeatureCapability::available(["alsa", "opus"])
            .with_selected_backend(Some("alsa".to_string())),
        rustdesk: FeatureCapability::available(["builtin"]),
        diagnostics: FeatureCapability::available(["android_linux"]),
        extensions: FeatureCapability::unsupported("unsupported on Android Amlogic v1"),
        service_installation: FeatureCapability::available(["android_foreground_service"]),
    }
}
