//! Extension management API handlers

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use typeshare::typeshare;

use crate::error::{AppError, Result};
use crate::extensions::{
    EasytierConfig, EasytierInfo, ExtensionId, ExtensionInfo, ExtensionLogs,
    ExtensionsStatus, GostcConfig, GostcInfo, TtydConfig, TtydInfo,
};
use crate::state::AppState;

// ============================================================================
// Get all extensions status
// ============================================================================

/// Get status of all extensions
/// GET /api/extensions
pub async fn list_extensions(State(state): State<Arc<AppState>>) -> Json<ExtensionsStatus> {
    let config = state.config.get();
    let mgr = &state.extensions;

    Json(ExtensionsStatus {
        ttyd: TtydInfo {
            available: mgr.check_available(ExtensionId::Ttyd),
            status: mgr.status(ExtensionId::Ttyd).await,
            config: config.extensions.ttyd.clone(),
        },
        gostc: GostcInfo {
            available: mgr.check_available(ExtensionId::Gostc),
            status: mgr.status(ExtensionId::Gostc).await,
            config: config.extensions.gostc.clone(),
        },
        easytier: EasytierInfo {
            available: mgr.check_available(ExtensionId::Easytier),
            status: mgr.status(ExtensionId::Easytier).await,
            config: config.extensions.easytier.clone(),
        },
    })
}

// ============================================================================
// Individual extension status
// ============================================================================

/// Get status of a single extension
/// GET /api/extensions/:id
pub async fn get_extension(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ExtensionInfo>> {
    let ext_id: ExtensionId = id
        .parse()
        .map_err(|_| AppError::NotFound(format!("Unknown extension: {}", id)))?;

    let mgr = &state.extensions;

    Ok(Json(ExtensionInfo {
        available: mgr.check_available(ext_id),
        status: mgr.status(ext_id).await,
    }))
}

// ============================================================================
// Start/Stop extensions
// ============================================================================

/// Start an extension
/// POST /api/extensions/:id/start
pub async fn start_extension(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ExtensionInfo>> {
    let ext_id: ExtensionId = id
        .parse()
        .map_err(|_| AppError::NotFound(format!("Unknown extension: {}", id)))?;

    let config = state.config.get();
    let mgr = &state.extensions;

    // Start the extension
    mgr.start(ext_id, &config.extensions)
        .await
        .map_err(|e| AppError::Internal(e))?;

    // Return updated status
    Ok(Json(ExtensionInfo {
        available: mgr.check_available(ext_id),
        status: mgr.status(ext_id).await,
    }))
}

/// Stop an extension
/// POST /api/extensions/:id/stop
pub async fn stop_extension(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ExtensionInfo>> {
    let ext_id: ExtensionId = id
        .parse()
        .map_err(|_| AppError::NotFound(format!("Unknown extension: {}", id)))?;

    let mgr = &state.extensions;

    // Stop the extension
    mgr.stop(ext_id)
        .await
        .map_err(|e| AppError::Internal(e))?;

    // Return updated status
    Ok(Json(ExtensionInfo {
        available: mgr.check_available(ext_id),
        status: mgr.status(ext_id).await,
    }))
}

// ============================================================================
// Extension logs
// ============================================================================

/// Query parameters for logs
#[derive(Deserialize, Default)]
pub struct LogsQuery {
    /// Number of lines to return (default: 100, max: 500)
    pub lines: Option<usize>,
}

/// Get extension logs
/// GET /api/extensions/:id/logs
pub async fn get_extension_logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<LogsQuery>,
) -> Result<Json<ExtensionLogs>> {
    let ext_id: ExtensionId = id
        .parse()
        .map_err(|_| AppError::NotFound(format!("Unknown extension: {}", id)))?;

    let lines = params.lines.unwrap_or(100).min(500);
    let logs = state.extensions.logs(ext_id, lines).await;

    Ok(Json(ExtensionLogs { id: ext_id, logs }))
}

// ============================================================================
// Update extension config
// ============================================================================

/// Update ttyd config
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct TtydConfigUpdate {
    pub enabled: Option<bool>,
    pub port: Option<u16>,
    pub shell: Option<String>,
    pub credential: Option<String>,
}

/// Update gostc config
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct GostcConfigUpdate {
    pub enabled: Option<bool>,
    pub addr: Option<String>,
    pub key: Option<String>,
    pub tls: Option<bool>,
}

/// Update easytier config
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct EasytierConfigUpdate {
    pub enabled: Option<bool>,
    pub network_name: Option<String>,
    pub network_secret: Option<String>,
    pub peer_urls: Option<Vec<String>>,
    pub virtual_ip: Option<String>,
}

/// Update ttyd configuration
/// PATCH /api/extensions/ttyd/config
pub async fn update_ttyd_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TtydConfigUpdate>,
) -> Result<Json<TtydConfig>> {
    // Get current config
    let was_enabled = state.config.get().extensions.ttyd.enabled;

    // Update config
    state
        .config
        .update(|config| {
            let ttyd = &mut config.extensions.ttyd;
            if let Some(enabled) = req.enabled {
                ttyd.enabled = enabled;
            }
            if let Some(port) = req.port {
                ttyd.port = port;
            }
            if let Some(ref shell) = req.shell {
                ttyd.shell = shell.clone();
            }
            if req.credential.is_some() {
                ttyd.credential = req.credential.clone();
            }
        })
        .await?;

    let new_config = state.config.get();
    let is_enabled = new_config.extensions.ttyd.enabled;

    // Handle enable/disable state change
    if was_enabled && !is_enabled {
        // Was running, now disabled - stop it
        state.extensions.stop(ExtensionId::Ttyd).await.ok();
    } else if !was_enabled && is_enabled {
        // Was disabled, now enabled - start it
        if state.extensions.check_available(ExtensionId::Ttyd) {
            state
                .extensions
                .start(ExtensionId::Ttyd, &new_config.extensions)
                .await
                .ok();
        }
    }

    Ok(Json(new_config.extensions.ttyd.clone()))
}

/// Update gostc configuration
/// PATCH /api/extensions/gostc/config
pub async fn update_gostc_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GostcConfigUpdate>,
) -> Result<Json<GostcConfig>> {
    let was_enabled = state.config.get().extensions.gostc.enabled;

    state
        .config
        .update(|config| {
            let gostc = &mut config.extensions.gostc;
            if let Some(enabled) = req.enabled {
                gostc.enabled = enabled;
            }
            if let Some(ref addr) = req.addr {
                gostc.addr = addr.clone();
            }
            if let Some(ref key) = req.key {
                gostc.key = key.clone();
            }
            if let Some(tls) = req.tls {
                gostc.tls = tls;
            }
        })
        .await?;

    let new_config = state.config.get();
    let is_enabled = new_config.extensions.gostc.enabled;
    let has_key = !new_config.extensions.gostc.key.is_empty();

    if was_enabled && !is_enabled {
        state.extensions.stop(ExtensionId::Gostc).await.ok();
    } else if !was_enabled && is_enabled && has_key {
        if state.extensions.check_available(ExtensionId::Gostc) {
            state
                .extensions
                .start(ExtensionId::Gostc, &new_config.extensions)
                .await
                .ok();
        }
    }

    Ok(Json(new_config.extensions.gostc.clone()))
}

/// Update easytier configuration
/// PATCH /api/extensions/easytier/config
pub async fn update_easytier_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EasytierConfigUpdate>,
) -> Result<Json<EasytierConfig>> {
    let was_enabled = state.config.get().extensions.easytier.enabled;

    state
        .config
        .update(|config| {
            let et = &mut config.extensions.easytier;
            if let Some(enabled) = req.enabled {
                et.enabled = enabled;
            }
            if let Some(ref name) = req.network_name {
                et.network_name = name.clone();
            }
            if let Some(ref secret) = req.network_secret {
                et.network_secret = secret.clone();
            }
            if let Some(ref peers) = req.peer_urls {
                et.peer_urls = peers.clone();
            }
            if req.virtual_ip.is_some() {
                et.virtual_ip = req.virtual_ip.clone();
            }
        })
        .await?;

    let new_config = state.config.get();
    let is_enabled = new_config.extensions.easytier.enabled;
    let has_name = !new_config.extensions.easytier.network_name.is_empty();

    if was_enabled && !is_enabled {
        state.extensions.stop(ExtensionId::Easytier).await.ok();
    } else if !was_enabled && is_enabled && has_name {
        if state.extensions.check_available(ExtensionId::Easytier) {
            state
                .extensions
                .start(ExtensionId::Easytier, &new_config.extensions)
                .await
                .ok();
        }
    }

    Ok(Json(new_config.extensions.easytier.clone()))
}

// ============================================================================
// Ttyd status for console (simplified)
// ============================================================================

/// Simple ttyd status for console view
#[typeshare]
#[derive(Debug, Serialize)]
pub struct TtydStatus {
    pub available: bool,
    pub running: bool,
}

/// Get ttyd status for console view
/// GET /api/extensions/ttyd/status
pub async fn get_ttyd_status(State(state): State<Arc<AppState>>) -> Json<TtydStatus> {
    let mgr = &state.extensions;
    let status = mgr.status(ExtensionId::Ttyd).await;

    Json(TtydStatus {
        available: mgr.check_available(ExtensionId::Ttyd),
        running: status.is_running(),
    })
}
