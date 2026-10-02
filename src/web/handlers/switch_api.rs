use super::*;

use crate::config::SwitchConfig;
use crate::switch::SWITCH_MAX_CHANNELS;

/// 供前端展示的单路可切换通道描述。
#[derive(Serialize)]
pub struct SwitchChannel {
    pub index: u8,
    pub label: String,
    pub active: bool,
}

/// KVM 切换器状态响应。
#[derive(Serialize)]
pub struct SwitchStatusResponse {
    pub available: bool,
    pub connected: bool,
    pub device: String,
    pub baud_rate: u32,
    pub channel_count: u8,
    pub current_channel: Option<u8>,
    pub channels: Vec<SwitchChannel>,
    pub error: Option<String>,
}

fn build_channels(config: &SwitchConfig, current: Option<u8>) -> Vec<SwitchChannel> {
    (1..=config.channel_count.min(SWITCH_MAX_CHANNELS))
        .map(|index| SwitchChannel {
            index,
            label: config.channel_label(index),
            active: current == Some(index),
        })
        .collect()
}

/// 查询 KVM 切换器状态。
pub async fn switch_status(State(state): State<Arc<AppState>>) -> Json<SwitchStatusResponse> {
    let config = state.config.get().switch.clone();
    let guard = state.switch.read().await;

    let (available, connected, device, baud_rate, channel_count, current, error) =
        match guard.as_ref() {
            Some(sw) => {
                let s = sw.state().await;
                (
                    s.available,
                    s.connected,
                    s.device,
                    s.baud_rate,
                    s.channel_count,
                    s.current_channel,
                    s.error,
                )
            }
            None => (
                config.enabled,
                false,
                config.device.clone(),
                config.baud_rate,
                config.channel_count,
                None,
                None,
            ),
        };

    Json(SwitchStatusResponse {
        available,
        connected,
        device,
        baud_rate,
        channel_count,
        current_channel: current,
        channels: build_channels(&config, current),
        error,
    })
}

/// 切换请求体。
#[derive(Deserialize)]
pub struct SwitchRequest {
    pub channel: u8,
}

/// 切换到指定通道。
pub async fn switch_channel(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SwitchRequest>,
) -> Result<Json<LoginResponse>> {
    let guard = state.switch.read().await;
    let sw = guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("KVM switch controller not initialized".to_string()))?;

    sw.switch_to_channel(req.channel).await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Switched KVM input to channel {}", req.channel)),
    }))
}
