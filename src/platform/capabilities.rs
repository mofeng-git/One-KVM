//! Runtime platform mode and feature capability reporting.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformMode {
    AndroidAmlogic,
    Linux,
    Windows,
}

impl PlatformMode {
    pub const fn current() -> Self {
        if cfg!(feature = "android") {
            Self::AndroidAmlogic
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::AndroidAmlogic => "Android Amlogic",
            Self::Linux => "Linux",
            Self::Windows => "Windows",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureCapability {
    pub available: bool,
    pub backends: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl FeatureCapability {
    pub fn available(backends: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let backends = backends.into_iter().map(Into::into).collect();
        Self {
            available: true,
            backends,
            selected_backend: None,
            reason: None,
        }
    }

    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            backends: Vec::new(),
            selected_backend: None,
            reason: Some(reason.into()),
        }
    }

    pub fn with_selected_backend(mut self, backend: Option<String>) -> Self {
        self.selected_backend = backend;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    pub mode: PlatformMode,
    pub mode_label: &'static str,
    pub video_capture: FeatureCapability,
    pub encoder: FeatureCapability,
    pub hid: FeatureCapability,
    pub atx: FeatureCapability,
    pub msd: FeatureCapability,
    pub otg: FeatureCapability,
    pub audio: FeatureCapability,
    pub rustdesk: FeatureCapability,
    pub diagnostics: FeatureCapability,
    pub extensions: FeatureCapability,
    pub service_installation: FeatureCapability,
}

impl PlatformCapabilities {
    pub fn current() -> Self {
        #[cfg(feature = "android")]
        {
            return crate::platform::android::capabilities();
        }
        #[cfg(windows)]
        {
            return crate::platform::windows::capabilities();
        }
        #[cfg(all(unix, not(feature = "android")))]
        {
            return crate::platform::linux::capabilities();
        }
    }
}
