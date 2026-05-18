//! Linux platform capabilities.

use super::{FeatureCapability, PlatformCapabilities, PlatformMode};

pub fn capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        mode: PlatformMode::Linux,
        mode_label: PlatformMode::Linux.label(),
        video_capture: FeatureCapability::available(["v4l2"]),
        encoder: FeatureCapability::available([
            "software", "vaapi", "nvenc", "qsv", "amf", "rkmpp", "v4l2m2m",
        ]),
        hid: FeatureCapability::available(["otg", "ch9329", "none"]),
        atx: FeatureCapability::available(["gpio", "usb_relay", "serial", "wol", "none"]),
        msd: FeatureCapability::available(["configfs"]),
        otg: FeatureCapability::available(["configfs"]),
        audio: FeatureCapability::available(["alsa"]),
        rustdesk: FeatureCapability::available(["builtin"]),
        diagnostics: FeatureCapability::available(["linux"]),
        extensions: FeatureCapability::available(["linux"]),
        service_installation: FeatureCapability::available(["systemd"]),
    }
}
