#[cfg(unix)]
use axum::{extract::DefaultBodyLimit, routing::delete};
use axum::{
    middleware,
    routing::{any, get, patch, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use super::audio_ws::audio_ws_handler;
use super::handlers;
use super::uac_ws::uac_audio_ws_handler;
use super::ws::ws_handler;
use crate::auth::auth_middleware;
use crate::hid::websocket::ws_hid_handler;
use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    let redfish_router = {
        let config = state.config.get();
        if config.redfish.enabled {
            Some(crate::redfish::routes::create_redfish_router(state.clone()))
        } else {
            None
        }
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no auth required)
    // Note: /info moved to user_routes for security (contains hostname, IPs, etc.)
    let public_routes = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/auth/login", post(handlers::login))
        .route("/auth/login/totp", post(handlers::login_totp))
        .route("/setup", get(handlers::setup_status))
        .route("/setup/init", post(handlers::setup_init));

    // Authenticated routes (all logged-in users)
    let user_routes = Router::new()
        .route("/info", get(handlers::system_info))
        .route("/auth/logout", post(handlers::logout))
        .route("/auth/check", get(handlers::auth_check))
        .route("/auth/password", post(handlers::change_password))
        .route("/auth/username", post(handlers::change_username))
        .route("/auth/totp", get(handlers::totp_status))
        .route(
            "/auth/totp/enrollment",
            post(handlers::begin_totp_enrollment),
        )
        .route(
            "/auth/totp/enrollment/confirm",
            post(handlers::confirm_totp_enrollment),
        )
        .route("/auth/totp/disable", post(handlers::disable_totp))
        .route("/devices", get(handlers::list_devices))
        // WebSocket endpoint for real-time events
        .route("/ws", any(ws_handler))
        // Stream control endpoints
        .route("/stream/status", get(handlers::stream_state))
        .route("/stream/start", post(handlers::stream_start))
        .route("/stream/stop", post(handlers::stream_stop))
        .route("/stream/mode", get(handlers::stream_mode_get))
        .route("/stream/mode", post(handlers::stream_mode_set))
        .route("/stream/bitrate", post(handlers::stream_set_bitrate))
        .route("/stream/codecs", get(handlers::stream_codecs_list))
        .route("/stream/constraints", get(handlers::stream_constraints_get))
        .route(
            "/video/encoder/self-check",
            get(handlers::video_encoder_self_check),
        )
        // WebRTC endpoints
        .route("/webrtc/session", post(handlers::webrtc_create_session))
        .route("/webrtc/offer", post(handlers::webrtc_offer))
        .route("/webrtc/ice", post(handlers::webrtc_ice_candidate))
        .route("/webrtc/ice-servers", get(handlers::webrtc_ice_servers))
        .route("/webrtc/status", get(handlers::webrtc_status))
        .route("/webrtc/close", post(handlers::webrtc_close_session))
        // HID endpoints
        .route("/hid/status", get(handlers::hid_status))
        .route(
            "/hid/ch9329/descriptor",
            get(handlers::hid_ch9329_descriptor),
        )
        .route("/hid/reset", post(handlers::hid_reset))
        // WebSocket HID endpoint (for MJPEG mode)
        .route("/ws/hid", any(ws_hid_handler))
        // Audio endpoints
        .route("/audio/status", get(handlers::audio_status))
        .route("/audio/start", post(handlers::start_audio_streaming))
        .route("/audio/stop", post(handlers::stop_audio_streaming))
        .route("/audio/quality", post(handlers::set_audio_quality))
        .route("/audio/device", post(handlers::select_audio_device))
        .route("/audio/devices", get(handlers::list_audio_devices))
        // Audio WebSocket endpoint
        .route("/ws/audio", any(audio_ws_handler))
        .route("/ws/uac-audio", any(uac_audio_ws_handler))
        // Configuration management (domain-separated endpoints)
        .route("/config", get(handlers::config::get_all_config))
        .route("/config/video", get(handlers::config::get_video_config))
        .route(
            "/config/video",
            patch(handlers::config::update_video_config),
        )
        .route("/config/stream", get(handlers::config::get_stream_config))
        .route(
            "/config/stream",
            patch(handlers::config::update_stream_config),
        )
        .route("/config/hid", get(handlers::config::get_hid_config))
        .route("/config/hid", patch(handlers::config::update_hid_config))
        .route("/config/atx", get(handlers::config::get_atx_config))
        .route("/config/atx", patch(handlers::config::update_atx_config))
        .route("/config/audio", get(handlers::config::get_audio_config))
        .route(
            "/config/audio",
            patch(handlers::config::update_audio_config),
        )
        // RustDesk configuration endpoints
        .route(
            "/config/rustdesk",
            get(handlers::config::get_rustdesk_config),
        )
        .route(
            "/config/rustdesk",
            patch(handlers::config::update_rustdesk_config),
        )
        .route(
            "/config/rustdesk/status",
            get(handlers::config::get_rustdesk_status),
        )
        .route(
            "/config/rustdesk/password",
            get(handlers::config::get_device_password),
        )
        .route(
            "/config/rustdesk/regenerate-id",
            post(handlers::config::regenerate_device_id),
        )
        .route(
            "/config/rustdesk/regenerate-password",
            post(handlers::config::regenerate_device_password),
        )
        .route(
            "/config/rustdesk/start",
            post(handlers::config::start_rustdesk_service),
        )
        .route(
            "/config/rustdesk/stop",
            post(handlers::config::stop_rustdesk_service),
        )
        // VNC configuration endpoints
        .route("/config/vnc", get(handlers::config::get_vnc_config))
        .route("/config/vnc", patch(handlers::config::update_vnc_config))
        .route("/config/vnc/status", get(handlers::config::get_vnc_status))
        .route(
            "/config/vnc/start",
            post(handlers::config::start_vnc_service),
        )
        .route("/config/vnc/stop", post(handlers::config::stop_vnc_service))
        // RTSP configuration endpoints
        .route("/config/rtsp", get(handlers::config::get_rtsp_config))
        .route("/config/rtsp", patch(handlers::config::update_rtsp_config))
        .route(
            "/config/rtsp/status",
            get(handlers::config::get_rtsp_status),
        )
        .route(
            "/config/rtsp/start",
            post(handlers::config::start_rtsp_service),
        )
        .route(
            "/config/rtsp/stop",
            post(handlers::config::stop_rtsp_service),
        )
        // Web server configuration
        .route("/config/web", get(handlers::config::get_web_config))
        .route("/config/web", patch(handlers::config::update_web_config))
        .route(
            "/config/watchdog",
            get(handlers::config::get_watchdog_config),
        )
        .route(
            "/config/watchdog",
            patch(handlers::config::update_watchdog_config),
        )
        .route("/config/computer-use", get(handlers::computer_use_config))
        .route(
            "/config/computer-use",
            patch(handlers::computer_use_update_config),
        )
        .route("/computer-use/session", get(handlers::computer_use_session))
        .route("/computer-use/session", post(handlers::computer_use_start))
        .route(
            "/computer-use/session/stop",
            post(handlers::computer_use_stop),
        )
        .route("/ws/computer-use", any(handlers::computer_use_ws))
        // Auth configuration
        .route("/config/auth", get(handlers::config::get_auth_config))
        .route("/config/auth", patch(handlers::config::update_auth_config))
        // Redfish configuration
        .route("/config/redfish", get(handlers::config::get_redfish_config))
        .route(
            "/config/redfish",
            patch(handlers::config::update_redfish_config),
        )
        // System control
        .route("/system/restart", post(handlers::system_restart))
        .route("/update/overview", get(handlers::update_overview))
        .route("/update/upgrade", post(handlers::update_upgrade))
        .route("/update/status", get(handlers::update_status))
        // ATX (Power Control) endpoints
        .route("/atx/status", get(handlers::atx_status))
        .route("/atx/power", post(handlers::atx_power))
        .route("/atx/wol", post(handlers::atx_wol))
        .route("/atx/wol/history", get(handlers::atx_wol_history))
        // Device discovery endpoints
        .route("/devices/atx", get(handlers::devices::list_atx_devices))
        // Extension management endpoints
        .route("/extensions", get(handlers::extensions::list_extensions))
        .route("/extensions/{id}", get(handlers::extensions::get_extension))
        .route(
            "/extensions/{id}/start",
            post(handlers::extensions::start_extension),
        )
        .route(
            "/extensions/{id}/stop",
            post(handlers::extensions::stop_extension),
        )
        .route(
            "/extensions/{id}/logs",
            get(handlers::extensions::get_extension_logs),
        )
        .route(
            "/extensions/ttyd/config",
            patch(handlers::extensions::update_ttyd_config),
        )
        .route(
            "/extensions/gostc/config",
            patch(handlers::extensions::update_gostc_config),
        )
        .route(
            "/extensions/easytier/config",
            patch(handlers::extensions::update_easytier_config),
        )
        .route(
            "/extensions/frpc/config",
            patch(handlers::extensions::update_frpc_config),
        )
        // Terminal (ttyd) reverse proxy - WebSocket and HTTP
        .route("/terminal", get(handlers::terminal::terminal_index))
        .route("/terminal/", get(handlers::terminal::terminal_index))
        .route("/terminal/ws", get(handlers::terminal::terminal_ws))
        .route("/terminal/{*path}", get(handlers::terminal::terminal_proxy));

    #[cfg(unix)]
    let user_routes = {
        user_routes
            .route("/hid/otg/self-check", get(handlers::hid_otg_self_check))
            .route("/config/msd", get(handlers::config::get_msd_config))
            .route("/config/msd", patch(handlers::config::update_msd_config))
            .route("/config/otg", patch(handlers::config::update_otg_config))
            .route(
                "/config/otg-network",
                get(handlers::config::get_otg_network_config),
            )
            .route(
                "/config/otg-network",
                patch(handlers::config::update_otg_network_config),
            )
            .route(
                "/otg/network/status",
                get(handlers::config::get_otg_network_status),
            )
            .route(
                "/config/uac",
                get(handlers::config::get_uac_config),
            )
            .route(
                "/config/uac",
                patch(handlers::config::update_uac_config),
            )
            .route("/msd/status", get(handlers::msd_status))
            .route("/msd/images", get(handlers::msd_images_list))
            .route("/msd/images/download", post(handlers::msd_image_download))
            .route(
                "/msd/images/download/cancel",
                post(handlers::msd_image_download_cancel),
            )
            .route("/msd/images/{id}", get(handlers::msd_image_get))
            .route("/msd/images/{id}", delete(handlers::msd_image_delete))
            .route("/msd/disk-mode", put(handlers::msd_disk_mode_put))
            .route("/msd/images/{id}/mount", post(handlers::msd_image_mount))
            .route(
                "/msd/images/{id}/mount",
                delete(handlers::msd_image_unmount),
            )
            .route("/msd/drive", get(handlers::msd_drive_info))
            .route("/msd/drive", delete(handlers::msd_drive_delete))
            .route("/msd/drive/mount", post(handlers::msd_drive_mount))
            .route("/msd/drive/mount", delete(handlers::msd_drive_unmount))
            .route("/msd/drive/init", post(handlers::msd_drive_init))
            .route("/msd/drive/files", get(handlers::msd_drive_files))
            .route(
                "/msd/drive/files/{*path}",
                get(handlers::msd_drive_download),
            )
            .route(
                "/msd/drive/files/{*path}",
                delete(handlers::msd_drive_file_delete),
            )
            .route("/msd/drive/mkdir/{*path}", post(handlers::msd_drive_mkdir))
            .route("/devices/usb", get(handlers::devices::list_usb_devices))
            .route(
                "/devices/network",
                get(handlers::devices::list_network_interfaces),
            )
            .route(
                "/devices/usb/reset",
                post(handlers::devices::reset_usb_device),
            )
    };

    // Protected routes (all authenticated users)
    let protected_routes = user_routes;

    // Stream endpoints (accessible with auth, but typically embedded in pages)
    let stream_routes = Router::new()
        .route("/stream", get(handlers::mjpeg_stream))
        .route("/stream/mjpeg", get(handlers::mjpeg_stream))
        .route("/snapshot", get(handlers::snapshot));

    // Large file upload routes (MSD images and drive files)
    // Use streaming upload to support files larger than available RAM
    // Disable body limit for streaming uploads - files are written directly to disk
    #[cfg(unix)]
    let upload_routes = Router::new()
        .route("/msd/images", post(handlers::msd_image_upload))
        .route("/msd/drive/files", post(handlers::msd_drive_upload))
        .layer(DefaultBodyLimit::disable());
    #[cfg(not(unix))]
    let upload_routes = Router::new();

    // Combine API routes
    let api_routes = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(stream_routes)
        .merge(upload_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Static file serving
    let static_routes = super::static_files::static_file_router();

    // Main router
    let main_router = Router::new()
        .nest("/api", api_routes)
        .merge(static_routes)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    match redfish_router {
        Some(rf) => main_router.merge(rf),
        None => main_router,
    }
}
