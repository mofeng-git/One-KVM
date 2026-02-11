//! 配置管理 Handler 模块
//!
//! 提供 RESTful 域分离的配置 API：
//! - GET  /api/config/video  - 获取视频配置
//! - PATCH /api/config/video - 更新视频配置
//! - GET  /api/config/stream - 获取流配置
//! - PATCH /api/config/stream - 更新流配置
//! - GET  /api/config/hid    - 获取 HID 配置
//! - PATCH /api/config/hid   - 更新 HID 配置
//! - GET  /api/config/msd    - 获取 MSD 配置
//! - PATCH /api/config/msd   - 更新 MSD 配置
//! - GET  /api/config/atx    - 获取 ATX 配置
//! - PATCH /api/config/atx   - 更新 ATX 配置
//! - GET  /api/config/audio  - 获取音频配置
//! - PATCH /api/config/audio - 更新音频配置
//! - GET  /api/config/rustdesk - 获取 RustDesk 配置
//! - PATCH /api/config/rustdesk - 更新 RustDesk 配置

pub(crate) mod apply;
mod types;

mod atx;
mod audio;
mod auth;
mod hid;
mod msd;
mod rustdesk;
mod rtsp;
mod stream;
pub(crate) mod video;
mod web;

// 导出 handler 函数
pub use atx::{get_atx_config, update_atx_config};
pub use audio::{get_audio_config, update_audio_config};
pub use auth::{get_auth_config, update_auth_config};
pub use hid::{get_hid_config, update_hid_config};
pub use msd::{get_msd_config, update_msd_config};
pub use rustdesk::{
    get_device_password, get_rustdesk_config, get_rustdesk_status, regenerate_device_id,
    regenerate_device_password, update_rustdesk_config,
};
pub use rtsp::{get_rtsp_config, get_rtsp_status, update_rtsp_config};
pub use stream::{get_stream_config, update_stream_config};
pub use video::{get_video_config, update_video_config};
pub use web::{get_web_config, update_web_config};

// 保留全局配置查询（向后兼容）
use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::state::AppState;

fn sanitize_config_for_api(config: &mut AppConfig) {
    // Auth secrets
    config.auth.totp_secret = None;

    // Stream secrets
    config.stream.turn_password = None;

    // RustDesk secrets
    config.rustdesk.device_password.clear();
    config.rustdesk.relay_key = None;
    config.rustdesk.public_key = None;
    config.rustdesk.private_key = None;
    config.rustdesk.signing_public_key = None;
    config.rustdesk.signing_private_key = None;

    // RTSP secrets
    config.rtsp.password = None;
}

/// 获取完整配置
pub async fn get_all_config(State(state): State<Arc<AppState>>) -> Json<AppConfig> {
    let mut config = (*state.config.get()).clone();
    // 不暴露敏感信息
    sanitize_config_for_api(&mut config);
    Json(config)
}
