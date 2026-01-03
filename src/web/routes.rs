use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use super::audio_ws::audio_ws_handler;
use super::handlers;
use super::ws::ws_handler;
use crate::auth::{auth_middleware, require_admin};
use crate::hid::websocket::ws_hid_handler;
use crate::state::AppState;

/// Create the main application router
pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no auth required)
    // Note: /info moved to user_routes for security (contains hostname, IPs, etc.)
    let public_routes = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/auth/login", post(handlers::login))
        .route("/setup", get(handlers::setup_status))
        .route("/setup/init", post(handlers::setup_init));

    // User routes (authenticated users - both regular and admin)
    let user_routes = Router::new()
        .route("/info", get(handlers::system_info))
        .route("/auth/logout", post(handlers::logout))
        .route("/auth/check", get(handlers::auth_check))
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
        // WebRTC endpoints
        .route("/webrtc/session", post(handlers::webrtc_create_session))
        .route("/webrtc/offer", post(handlers::webrtc_offer))
        .route("/webrtc/ice", post(handlers::webrtc_ice_candidate))
        .route("/webrtc/ice-servers", get(handlers::webrtc_ice_servers))
        .route("/webrtc/status", get(handlers::webrtc_status))
        .route("/webrtc/close", post(handlers::webrtc_close_session))
        // HID endpoints
        .route("/hid/status", get(handlers::hid_status))
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
        // User can change their own password (handler will check ownership)
        .route("/users/:id/password", post(handlers::change_user_password));

    // Admin-only routes (require admin privileges)
    let admin_routes = Router::new()
        // Configuration management (domain-separated endpoints)
        .route("/config", get(handlers::config::get_all_config))
        .route("/config", post(handlers::update_config))
        .route("/config/video", get(handlers::config::get_video_config))
        .route("/config/video", patch(handlers::config::update_video_config))
        .route("/config/stream", get(handlers::config::get_stream_config))
        .route("/config/stream", patch(handlers::config::update_stream_config))
        .route("/config/hid", get(handlers::config::get_hid_config))
        .route("/config/hid", patch(handlers::config::update_hid_config))
        .route("/config/msd", get(handlers::config::get_msd_config))
        .route("/config/msd", patch(handlers::config::update_msd_config))
        .route("/config/atx", get(handlers::config::get_atx_config))
        .route("/config/atx", patch(handlers::config::update_atx_config))
        .route("/config/audio", get(handlers::config::get_audio_config))
        .route("/config/audio", patch(handlers::config::update_audio_config))
        // RustDesk configuration endpoints
        .route("/config/rustdesk", get(handlers::config::get_rustdesk_config))
        .route("/config/rustdesk", patch(handlers::config::update_rustdesk_config))
        .route("/config/rustdesk/status", get(handlers::config::get_rustdesk_status))
        .route("/config/rustdesk/password", get(handlers::config::get_device_password))
        .route("/config/rustdesk/regenerate-id", post(handlers::config::regenerate_device_id))
        .route("/config/rustdesk/regenerate-password", post(handlers::config::regenerate_device_password))
        // Web server configuration
        .route("/config/web", get(handlers::config::get_web_config))
        .route("/config/web", patch(handlers::config::update_web_config))
        // System control
        .route("/system/restart", post(handlers::system_restart))
        // MSD (Mass Storage Device) endpoints
        .route("/msd/status", get(handlers::msd_status))
        .route("/msd/images", get(handlers::msd_images_list))
        .route("/msd/images/download", post(handlers::msd_image_download))
        .route("/msd/images/download/cancel", post(handlers::msd_image_download_cancel))
        .route("/msd/images/:id", get(handlers::msd_image_get))
        .route("/msd/images/:id", delete(handlers::msd_image_delete))
        .route("/msd/connect", post(handlers::msd_connect))
        .route("/msd/disconnect", post(handlers::msd_disconnect))
        // MSD Virtual Drive endpoints
        .route("/msd/drive", get(handlers::msd_drive_info))
        .route("/msd/drive", delete(handlers::msd_drive_delete))
        .route("/msd/drive/init", post(handlers::msd_drive_init))
        .route("/msd/drive/files", get(handlers::msd_drive_files))
        .route("/msd/drive/files/*path", get(handlers::msd_drive_download))
        .route("/msd/drive/files/*path", delete(handlers::msd_drive_file_delete))
        .route("/msd/drive/mkdir/*path", post(handlers::msd_drive_mkdir))
        // ATX (Power Control) endpoints
        .route("/atx/status", get(handlers::atx_status))
        .route("/atx/power", post(handlers::atx_power))
        .route("/atx/wol", post(handlers::atx_wol))
        // Device discovery endpoints
        .route("/devices/atx", get(handlers::devices::list_atx_devices))
        // User management endpoints
        .route("/users", get(handlers::list_users))
        .route("/users", post(handlers::create_user))
        .route("/users/:id", put(handlers::update_user))
        .route("/users/:id", delete(handlers::delete_user))
        // Extension management endpoints
        .route("/extensions", get(handlers::extensions::list_extensions))
        .route("/extensions/:id", get(handlers::extensions::get_extension))
        .route("/extensions/:id/start", post(handlers::extensions::start_extension))
        .route("/extensions/:id/stop", post(handlers::extensions::stop_extension))
        .route("/extensions/:id/logs", get(handlers::extensions::get_extension_logs))
        .route("/extensions/ttyd/config", patch(handlers::extensions::update_ttyd_config))
        .route("/extensions/ttyd/status", get(handlers::extensions::get_ttyd_status))
        .route("/extensions/gostc/config", patch(handlers::extensions::update_gostc_config))
        .route("/extensions/easytier/config", patch(handlers::extensions::update_easytier_config))
        // Terminal (ttyd) reverse proxy - WebSocket and HTTP
        .route("/terminal", get(handlers::terminal::terminal_index))
        .route("/terminal/", get(handlers::terminal::terminal_index))
        .route("/terminal/ws", get(handlers::terminal::terminal_ws))
        .route("/terminal/*path", get(handlers::terminal::terminal_proxy))
        // Apply admin middleware to all admin routes
        .layer(middleware::from_fn_with_state(state.clone(), require_admin));

    // Combine protected routes (user + admin)
    let protected_routes = Router::new()
        .merge(user_routes)
        .merge(admin_routes);

    // Stream endpoints (accessible with auth, but typically embedded in pages)
    let stream_routes = Router::new()
        .route("/stream", get(handlers::mjpeg_stream))
        .route("/stream/mjpeg", get(handlers::mjpeg_stream))
        .route("/snapshot", get(handlers::snapshot));

    // Large file upload routes (MSD images and drive files)
    // Use streaming upload to support files larger than available RAM
    // Disable body limit for streaming uploads - files are written directly to disk
    let upload_routes = Router::new()
        .route("/msd/images", post(handlers::msd_image_upload))
        .route("/msd/drive/files", post(handlers::msd_drive_upload))
        .layer(DefaultBodyLimit::disable());

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
    Router::new()
        .nest("/api", api_routes)
        .merge(static_routes)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
