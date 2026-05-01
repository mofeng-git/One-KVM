pub(crate) mod apply;
mod types;

mod atx;
mod audio;
mod auth;
mod hid;
mod msd;
mod rtsp;
mod rustdesk;
mod stream;
pub(crate) mod video;
mod web;

pub use atx::{get_atx_config, update_atx_config};
pub use audio::{get_audio_config, update_audio_config};
pub use auth::{get_auth_config, update_auth_config};
pub use hid::{get_hid_config, update_hid_config};
pub use msd::{get_msd_config, update_msd_config};
pub use rtsp::{get_rtsp_config, get_rtsp_status, update_rtsp_config};
pub use rustdesk::{
    get_device_password, get_rustdesk_config, get_rustdesk_status, regenerate_device_id,
    regenerate_device_password, update_rustdesk_config,
};
pub use stream::{get_stream_config, update_stream_config};
pub use video::{get_video_config, update_video_config};
pub use web::{get_web_config, update_web_config};

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::state::AppState;

fn sanitize_config_for_api(config: &mut AppConfig) {
    config.auth.totp_secret = None;

    config.stream.turn_password = None;

    config.rustdesk.device_password.clear();
    config.rustdesk.relay_key = None;
    config.rustdesk.public_key = None;
    config.rustdesk.private_key = None;
    config.rustdesk.signing_public_key = None;
    config.rustdesk.signing_private_key = None;

    config.rtsp.password = None;
}

pub async fn get_all_config(State(state): State<Arc<AppState>>) -> Json<AppConfig> {
    let mut config = (*state.config.get()).clone();
    sanitize_config_for_api(&mut config);
    Json(config)
}
