use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use toml_edit::DocumentMut;
use typeshare::typeshare;

use crate::error::{AppError, Result};
use crate::extensions::{
    EasytierConfig, EasytierInfo, ExtensionId, ExtensionInfo, ExtensionLogs, ExtensionsStatus,
    FrpProxyType, FrpcConfig, FrpcConfigMode, FrpcInfo, GostcConfig, GostcInfo, TtydConfig,
    TtydInfo,
};
use crate::state::AppState;

fn validate_gostc_enabled(config: &GostcConfig) -> Result<()> {
    if config.addr.trim().is_empty() {
        return Err(AppError::BadRequest(
            "GOSTC server address is required".into(),
        ));
    }
    if config.key.is_empty() {
        return Err(AppError::BadRequest("GOSTC client key is required".into()));
    }
    Ok(())
}

fn validate_easytier_enabled(config: &EasytierConfig) -> Result<()> {
    if config.network_name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "EasyTier network name is required".into(),
        ));
    }
    Ok(())
}

fn validate_frpc_enabled(config: &FrpcConfig) -> Result<()> {
    match config.config_mode {
        FrpcConfigMode::Quick => {
            if config.proxy_name.trim().is_empty() {
                return Err(AppError::BadRequest("FRPC proxy name is required".into()));
            }
            if config.server_addr.trim().is_empty() {
                return Err(AppError::BadRequest(
                    "FRPC server address is required".into(),
                ));
            }
            if config.token.is_empty() {
                return Err(AppError::BadRequest("FRPC token is required".into()));
            }
            if config.local_ip.trim().is_empty() {
                return Err(AppError::BadRequest("FRPC local IP is required".into()));
            }
            if matches!(config.proxy_type, FrpProxyType::Tcp | FrpProxyType::Udp)
                && config.remote_port.is_none()
            {
                return Err(AppError::BadRequest(
                    "FRPC remote port is required for TCP/UDP proxies".into(),
                ));
            }
        }
        FrpcConfigMode::Full => {
            let toml = config.custom_toml.trim();
            if toml.is_empty() {
                return Err(AppError::BadRequest(
                    "FRPC full configuration is required".into(),
                ));
            }
            toml.parse::<DocumentMut>().map_err(|e| {
                AppError::BadRequest(format!("FRPC full configuration is not valid TOML: {}", e))
            })?;
        }
    }
    Ok(())
}

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
        frpc: FrpcInfo {
            available: mgr.check_available(ExtensionId::Frpc),
            status: mgr.status(ExtensionId::Frpc).await,
            config: config.extensions.frpc.clone(),
        },
    })
}

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

pub async fn start_extension(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ExtensionInfo>> {
    let ext_id: ExtensionId = id
        .parse()
        .map_err(|_| AppError::NotFound(format!("Unknown extension: {}", id)))?;

    let config = state.config.get();
    let mgr = &state.extensions;

    mgr.start(ext_id, &config.extensions)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(ExtensionInfo {
        available: mgr.check_available(ext_id),
        status: mgr.status(ext_id).await,
    }))
}

pub async fn stop_extension(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ExtensionInfo>> {
    let ext_id: ExtensionId = id
        .parse()
        .map_err(|_| AppError::NotFound(format!("Unknown extension: {}", id)))?;

    let mgr = &state.extensions;

    mgr.stop(ext_id).await.map_err(AppError::Internal)?;

    Ok(Json(ExtensionInfo {
        available: mgr.check_available(ext_id),
        status: mgr.status(ext_id).await,
    }))
}

#[derive(Deserialize, Default)]
pub struct LogsQuery {
    /// Number of lines to return (default: 100, max: 500)
    pub lines: Option<usize>,
}

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

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct TtydConfigUpdate {
    pub enabled: Option<bool>,
    pub shell: Option<String>,
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct GostcConfigUpdate {
    pub enabled: Option<bool>,
    pub addr: Option<String>,
    pub key: Option<String>,
    pub tls: Option<bool>,
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct EasytierConfigUpdate {
    pub enabled: Option<bool>,
    pub network_name: Option<String>,
    pub network_secret: Option<String>,
    pub peer_urls: Option<Vec<String>>,
    pub virtual_ip: Option<String>,
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct FrpcConfigUpdate {
    pub enabled: Option<bool>,
    pub config_mode: Option<FrpcConfigMode>,
    pub proxy_name: Option<String>,
    pub proxy_type: Option<FrpProxyType>,
    pub server_addr: Option<String>,
    pub server_port: Option<u16>,
    pub token: Option<String>,
    pub local_ip: Option<String>,
    pub local_port: Option<u16>,
    pub remote_port: Option<Option<u16>>,
    pub custom_domain: Option<Option<String>>,
    pub secret_key: Option<String>,
    pub tls: Option<bool>,
    pub custom_toml: Option<String>,
}

pub async fn update_ttyd_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TtydConfigUpdate>,
) -> Result<Json<TtydConfig>> {
    let was_enabled = state.config.get().extensions.ttyd.enabled;

    state
        .config
        .update(|config| {
            let ttyd = &mut config.extensions.ttyd;
            if let Some(enabled) = req.enabled {
                ttyd.enabled = enabled;
            }
            if let Some(ref shell) = req.shell {
                ttyd.shell = shell.clone();
            }
        })
        .await?;

    let new_config = state.config.get();
    let is_enabled = new_config.extensions.ttyd.enabled;

    if was_enabled && !is_enabled {
        state.extensions.stop(ExtensionId::Ttyd).await.ok();
    } else if !was_enabled && is_enabled {
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

pub async fn update_gostc_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GostcConfigUpdate>,
) -> Result<Json<GostcConfig>> {
    let current_config = state.config.get();
    let was_enabled = current_config.extensions.gostc.enabled;
    let mut next_gostc = current_config.extensions.gostc.clone();

    if let Some(enabled) = req.enabled {
        next_gostc.enabled = enabled;
    }
    if let Some(ref addr) = req.addr {
        next_gostc.addr = addr.clone();
    }
    if let Some(ref key) = req.key {
        next_gostc.key = key.clone();
    }
    if let Some(tls) = req.tls {
        next_gostc.tls = tls;
    }

    if next_gostc.enabled {
        validate_gostc_enabled(&next_gostc)?;
    }

    state
        .config
        .update(|config| {
            config.extensions.gostc = next_gostc.clone();
        })
        .await?;

    let new_config = state.config.get();
    let is_enabled = new_config.extensions.gostc.enabled;

    if was_enabled && !is_enabled {
        state.extensions.stop(ExtensionId::Gostc).await.ok();
    } else if !was_enabled && is_enabled && state.extensions.check_available(ExtensionId::Gostc) {
        state
            .extensions
            .start(ExtensionId::Gostc, &new_config.extensions)
            .await
            .ok();
    }

    Ok(Json(new_config.extensions.gostc.clone()))
}

pub async fn update_easytier_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EasytierConfigUpdate>,
) -> Result<Json<EasytierConfig>> {
    let current_config = state.config.get();
    let was_enabled = current_config.extensions.easytier.enabled;
    let mut next_easytier = current_config.extensions.easytier.clone();

    if let Some(enabled) = req.enabled {
        next_easytier.enabled = enabled;
    }
    if let Some(ref name) = req.network_name {
        next_easytier.network_name = name.clone();
    }
    if let Some(ref secret) = req.network_secret {
        next_easytier.network_secret = secret.clone();
    }
    if let Some(ref peers) = req.peer_urls {
        next_easytier.peer_urls = peers.clone();
    }
    if req.virtual_ip.is_some() {
        next_easytier.virtual_ip = req.virtual_ip.clone();
    }

    if next_easytier.enabled {
        validate_easytier_enabled(&next_easytier)?;
    }

    state
        .config
        .update(|config| {
            config.extensions.easytier = next_easytier.clone();
        })
        .await?;

    let new_config = state.config.get();
    let is_enabled = new_config.extensions.easytier.enabled;

    if was_enabled && !is_enabled {
        state.extensions.stop(ExtensionId::Easytier).await.ok();
    } else if !was_enabled && is_enabled && state.extensions.check_available(ExtensionId::Easytier)
    {
        state
            .extensions
            .start(ExtensionId::Easytier, &new_config.extensions)
            .await
            .ok();
    }

    Ok(Json(new_config.extensions.easytier.clone()))
}

pub async fn update_frpc_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FrpcConfigUpdate>,
) -> Result<Json<FrpcConfig>> {
    let current_config = state.config.get();
    let was_enabled = current_config.extensions.frpc.enabled;
    let mut next_frpc = current_config.extensions.frpc.clone();

    if let Some(enabled) = req.enabled {
        next_frpc.enabled = enabled;
    }
    if let Some(config_mode) = req.config_mode {
        next_frpc.config_mode = config_mode;
    }
    if let Some(ref proxy_name) = req.proxy_name {
        next_frpc.proxy_name = proxy_name.clone();
    }
    if let Some(proxy_type) = req.proxy_type {
        next_frpc.proxy_type = proxy_type;
    }
    if let Some(ref addr) = req.server_addr {
        next_frpc.server_addr = addr.clone();
    }
    if let Some(port) = req.server_port {
        next_frpc.server_port = port;
    }
    if let Some(ref token) = req.token {
        next_frpc.token = token.clone();
    }
    if let Some(ref local_ip) = req.local_ip {
        next_frpc.local_ip = local_ip.clone();
    }
    if let Some(local_port) = req.local_port {
        next_frpc.local_port = local_port;
    }
    if let Some(remote_port) = req.remote_port {
        next_frpc.remote_port = remote_port;
    }
    if let Some(custom_domain) = req.custom_domain {
        next_frpc.custom_domain = custom_domain;
    }
    if let Some(ref secret_key) = req.secret_key {
        next_frpc.secret_key = secret_key.clone();
    }
    if let Some(tls) = req.tls {
        next_frpc.tls = tls;
    }
    if let Some(ref custom_toml) = req.custom_toml {
        next_frpc.custom_toml = custom_toml.clone();
    }

    if next_frpc.enabled || matches!(next_frpc.config_mode, FrpcConfigMode::Full) {
        validate_frpc_enabled(&next_frpc)?;
    }

    state
        .config
        .update(|config| {
            config.extensions.frpc = next_frpc.clone();
        })
        .await?;

    let new_config = state.config.get();
    let is_enabled = new_config.extensions.frpc.enabled;

    if was_enabled && !is_enabled {
        state.extensions.stop(ExtensionId::Frpc).await.ok();
    } else if !was_enabled && is_enabled && state.extensions.check_available(ExtensionId::Frpc) {
        state
            .extensions
            .start(ExtensionId::Frpc, &new_config.extensions)
            .await
            .ok();
    }

    Ok(Json(new_config.extensions.frpc.clone()))
}
