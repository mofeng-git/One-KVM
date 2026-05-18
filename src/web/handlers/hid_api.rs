use super::*;

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
