pub mod config;
pub mod devices;
pub mod extensions;
pub mod terminal;

mod account;
mod atx_api;
mod audio_api;
mod auth;
mod hid_api;
mod inventory;
#[cfg(unix)]
mod msd_api;
mod setup;
mod stream;
mod system;
mod update_api;
mod webrtc;

pub use account::*;
pub use atx_api::*;
pub use audio_api::*;
pub use auth::*;
pub use hid_api::*;
pub use inventory::*;
#[cfg(unix)]
pub use msd_api::*;
pub use setup::*;
pub use stream::*;
pub use system::*;
pub use update_api::*;
pub use webrtc::*;

use axum::{
    extract::{Query, State},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

use self::config::apply::ConfigApplyOptions;
use crate::auth::{Session, SESSION_COOKIE};
use crate::config::StreamMode;
use crate::diagnostics::{get_device_info, get_disk_space, DeviceInfo, DiskSpaceInfo};
use crate::error::{AppError, Result};
use crate::platform::PlatformCapabilities;
use crate::state::AppState;
use crate::update::{UpdateChannel, UpdateOverviewResponse, UpdateStatusResponse, UpgradeRequest};
use crate::utils::list_serial_ports;
use crate::video::codec::{
    build_hardware_self_check_runtime_error, run_hardware_self_check, BitratePreset,
    VideoEncoderSelfCheckResponse,
};
use crate::video::codec_constraints::codec_to_id;
