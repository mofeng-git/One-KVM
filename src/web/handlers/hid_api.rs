use super::*;
use crate::error::AppError;

#[derive(Deserialize)]
pub struct Ch9329DescriptorQuery {
    pub port: Option<String>,
    pub baud_rate: Option<u32>,
}

#[derive(Serialize)]
pub struct HidStatus {
    pub available: bool,
    pub backend: String,
    pub initialized: bool,
    pub online: bool,
    pub supports_absolute_mouse: bool,
    pub keyboard_leds_enabled: bool,
    pub led_state: crate::hid::LedState,
    pub screen_resolution: Option<(u32, u32)>,
    pub device: Option<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

/// OTG self-check status for troubleshooting USB gadget issues
#[cfg(unix)]
pub async fn hid_otg_self_check(
    State(state): State<Arc<AppState>>,
) -> Json<crate::otg::self_check::OtgSelfCheckResponse> {
    let config = state.config.get();
    Json(crate::otg::self_check::run(config.as_ref()))
}

/// Get HID status
pub async fn hid_status(State(state): State<Arc<AppState>>) -> Json<HidStatus> {
    let hid = state.hid.snapshot().await;
    Json(HidStatus {
        available: hid.available,
        backend: hid.backend,
        initialized: hid.initialized,
        online: hid.online,
        supports_absolute_mouse: hid.supports_absolute_mouse,
        keyboard_leds_enabled: hid.keyboard_leds_enabled,
        led_state: hid.led_state,
        screen_resolution: hid.screen_resolution,
        device: hid.device,
        error: hid.error,
        error_code: hid.error_code,
    })
}

/// Reset HID state
pub async fn hid_reset(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    state.hid.reset().await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("HID state reset".to_string()),
    }))
}

/// Read the CH9329 USB descriptor, falling back to the saved config when SET is not low.
pub async fn hid_ch9329_descriptor(
    State(state): State<Arc<AppState>>,
    Query(query): Query<Ch9329DescriptorQuery>,
) -> Result<Json<crate::config::Ch9329DescriptorState>> {
    let config = state.config.get();
    let hid = &config.hid;
    let port = query.port.as_deref().filter(|port| !port.trim().is_empty());
    let baud_rate = query.baud_rate;

    let descriptor_result = match (port, baud_rate) {
        (Some(port), Some(baud_rate))
            if port != hid.ch9329_port || baud_rate != hid.ch9329_baudrate =>
        {
            crate::hid::ch9329::Ch9329Backend::read_device_descriptor(port, baud_rate)
        }
        _ => state.hid.read_ch9329_descriptor().await,
    };

    let descriptor = match descriptor_result {
        Ok(descriptor) => descriptor,
        Err(err) if is_ch9329_config_mode_unavailable(&err) => cached_ch9329_descriptor(hid),
        Err(err) => return Err(err),
    };
    Ok(Json(descriptor))
}

fn is_ch9329_config_mode_unavailable(err: &AppError) -> bool {
    matches!(
        err,
        AppError::HidError {
            backend,
            error_code,
            ..
        } if backend == "ch9329" && error_code == "invalid_response"
    )
}

fn cached_ch9329_descriptor(
    hid: &crate::config::HidConfig,
) -> crate::config::Ch9329DescriptorState {
    let descriptor = hid.ch9329_descriptor.clone();
    crate::config::Ch9329DescriptorState {
        manufacturer_enabled: !descriptor.manufacturer.is_empty(),
        product_enabled: !descriptor.product.is_empty(),
        serial_enabled: descriptor
            .serial_number
            .as_ref()
            .is_some_and(|value| !value.is_empty()),
        config_mode_available: false,
        descriptor,
    }
}
