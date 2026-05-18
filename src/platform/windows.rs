//! Windows platform capabilities.

use super::{FeatureCapability, PlatformCapabilities, PlatformMode};

pub fn capabilities() -> PlatformCapabilities {
    let linux_only = "unsupported on Windows";
    PlatformCapabilities {
        mode: PlatformMode::Windows,
        mode_label: PlatformMode::Windows.label(),
        video_capture: FeatureCapability::available(["directshow_uvc", "mjpeg"])
            .with_selected_backend(Some("directshow_uvc".to_string())),
        encoder: FeatureCapability::available([
            "ffmpeg_h264",
            "ffmpeg_h265",
            "ffmpeg_vp8",
            "ffmpeg_vp9",
            "software",
            "mjpeg",
        ]),
        hid: FeatureCapability::available(["ch9329", "none"])
            .with_selected_backend(Some("ch9329".to_string())),
        atx: FeatureCapability::available(["serial", "wol", "none"]),
        msd: FeatureCapability::unsupported(linux_only),
        otg: FeatureCapability::unsupported(linux_only),
        audio: FeatureCapability::available(["wasapi", "opus"])
            .with_selected_backend(Some("wasapi".to_string())),
        rustdesk: FeatureCapability::available(["builtin", "tcp_direct", "relay"])
            .with_selected_backend(Some("builtin".to_string())),
        diagnostics: FeatureCapability::available(["windows"]),
        extensions: FeatureCapability::available(["windows_safe"]),
        service_installation: FeatureCapability::available(["windows_service"]),
    }
}
