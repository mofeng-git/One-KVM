use super::*;

use crate::atx::{AtxState, PowerStatus};

const WOL_HISTORY_DEFAULT_LIMIT: usize = 5;
const WOL_HISTORY_MAX_LIMIT: usize = 50;

/// ATX state response
#[derive(Serialize)]
pub struct AtxStateResponse {
    pub available: bool,
    pub backend: String,
    pub initialized: bool,
    pub power_status: String,
    pub led_supported: bool,
}

impl From<AtxState> for AtxStateResponse {
    fn from(state: AtxState) -> Self {
        Self {
            available: state.available,
            backend: if state.power_configured || state.reset_configured {
                format!(
                    "power: {}, reset: {}",
                    if state.power_configured { "yes" } else { "no" },
                    if state.reset_configured { "yes" } else { "no" }
                )
            } else {
                "none".to_string()
            },
            initialized: state.power_configured || state.reset_configured,
            power_status: match state.power_status {
                PowerStatus::On => "on".to_string(),
                PowerStatus::Off => "off".to_string(),
                PowerStatus::Unknown => "unknown".to_string(),
            },
            led_supported: state.led_supported,
        }
    }
}

/// Get ATX status
pub async fn atx_status(State(state): State<Arc<AppState>>) -> Result<Json<AtxStateResponse>> {
    let atx_guard = state.atx.read().await;

    match atx_guard.as_ref() {
        Some(atx) => {
            let atx_state = atx.state().await;
            Ok(Json(AtxStateResponse::from(atx_state)))
        }
        None => Ok(Json(AtxStateResponse {
            available: false,
            backend: "none".to_string(),
            initialized: false,
            power_status: "unknown".to_string(),
            led_supported: false,
        })),
    }
}

/// ATX power control request
#[derive(Deserialize)]
pub struct AtxPowerControlRequest {
    pub action: String, // "short", "long", "reset"
}

/// Control ATX power
pub async fn atx_power(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AtxPowerControlRequest>,
) -> Result<Json<LoginResponse>> {
    let atx_guard = state.atx.read().await;
    let atx = atx_guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("ATX controller not initialized".to_string()))?;

    match req.action.as_str() {
        "short" => {
            atx.power_short().await?;
            Ok(Json(LoginResponse {
                success: true,
                message: Some("Power short press executed".to_string()),
            }))
        }
        "long" => {
            atx.power_long().await?;
            Ok(Json(LoginResponse {
                success: true,
                message: Some("Power long press (force off) executed".to_string()),
            }))
        }
        "reset" => {
            atx.reset().await?;
            Ok(Json(LoginResponse {
                success: true,
                message: Some("Reset button pressed".to_string()),
            }))
        }
        _ => Err(AppError::BadRequest(format!(
            "Unknown ATX action: {}. Valid actions: short, long, reset",
            req.action
        ))),
    }
}

/// WOL request body
#[derive(Debug, Deserialize)]
pub struct WolRequest {
    /// Target MAC address (e.g., "AA:BB:CC:DD:EE:FF" or "AA-BB-CC-DD-EE-FF")
    pub mac_address: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct WolHistoryQuery {
    /// Maximum history entries to return
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct WolHistoryEntry {
    pub mac_address: String,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct WolHistoryResponse {
    pub history: Vec<WolHistoryEntry>,
}

fn normalize_wol_mac_address(mac_address: &str) -> String {
    let normalized = mac_address.trim().to_uppercase().replace('-', ":");

    if normalized.len() == 12 && normalized.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut mac_with_separator = String::with_capacity(17);
        for (index, chunk) in normalized.as_bytes().chunks(2).enumerate() {
            if index > 0 {
                mac_with_separator.push(':');
            }
            mac_with_separator.push(chunk[0] as char);
            mac_with_separator.push(chunk[1] as char);
        }
        mac_with_separator
    } else {
        normalized
    }
}

/// Send Wake-on-LAN magic packet
pub async fn atx_wol(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WolRequest>,
) -> Result<Json<LoginResponse>> {
    let mac_address = normalize_wol_mac_address(&req.mac_address);

    // Get WOL interface from config
    let config = state.config.get();
    let interface = if config.atx.wol_interface.is_empty() {
        None
    } else {
        Some(config.atx.wol_interface.as_str())
    };

    // Send WOL packet
    crate::atx::send_wol(&mac_address, interface)?;

    if let Err(error) = crate::atx::record_wol_history(state.db.pool(), &mac_address).await {
        warn!("Failed to persist WOL history: {}", error);
    }

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("WOL packet sent to {}", mac_address)),
    }))
}

/// Get WOL history
pub async fn atx_wol_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WolHistoryQuery>,
) -> Result<Json<WolHistoryResponse>> {
    let limit = query
        .limit
        .unwrap_or(WOL_HISTORY_DEFAULT_LIMIT)
        .clamp(1, WOL_HISTORY_MAX_LIMIT);

    let rows = crate::atx::list_wol_history(state.db.pool(), limit).await?;

    let history = rows
        .into_iter()
        .map(|(mac_address, updated_at)| WolHistoryEntry {
            mac_address,
            updated_at,
        })
        .collect();

    Ok(Json(WolHistoryResponse { history }))
}
