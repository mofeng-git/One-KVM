//! 外观设置（加载画面样式/背景、无信号图片）。
//!
//! 存成独立的小 JSON（`/etc/one-kvm/appearance.json`），不写进配置库：
//! 避免动 AppConfig 结构导致数据库迁移，也避免"改个外观就重启视频流"。

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

const PATH: &str = "/etc/one-kvm/appearance.json";
/// 单张图片 data URL 上限（约 1.5MB 原图），防止把内存撑爆
pub const MAX_IMAGE_CHARS: usize = 2 * 1024 * 1024;

/// 无信号检测（彩条触发条件）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SignalProbeSettings {
    /// auto（默认） | off | custom
    pub preset: Option<String>,
    pub vid: Option<String>,
    pub pid: Option<String>,
    pub register: Option<String>,
    /// 有信号时寄存器的读数
    pub locked_value: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    /// minimal（点阵+转圈） | branded（深色渐变+Logo） | custom（自定义背景）
    pub loading_style: Option<String>,
    /// data:image/jpeg;base64,... —— 刷新/连接时的背景
    pub loading_background: Option<String>,
    /// data:image/jpeg;base64,... —— 无信号时整帧显示
    pub no_signal_image: Option<String>,
    /// 无信号检测设置
    pub signal: Option<SignalProbeSettings>,
}

static CACHE: OnceLock<RwLock<Arc<AppearanceSettings>>> = OnceLock::new();

fn cache() -> &'static RwLock<Arc<AppearanceSettings>> {
    CACHE.get_or_init(|| RwLock::new(Arc::new(load_from_disk())))
}

fn load_from_disk() -> AppearanceSettings {
    std::fs::read_to_string(PATH)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// 当前外观设置
pub fn current() -> Arc<AppearanceSettings> {
    cache().read().clone()
}

pub fn validate(req: &AppearanceSettings) -> Result<(), String> {
    for (name, value) in [
        ("loading_background", &req.loading_background),
        ("no_signal_image", &req.no_signal_image),
    ] {
        if let Some(v) = value {
            if v.len() > MAX_IMAGE_CHARS {
                return Err(format!(
                    "{name} 太大：{} 字节（上限 {MAX_IMAGE_CHARS}）",
                    v.len()
                ));
            }
            if !v.is_empty() && !v.starts_with("data:image/") {
                return Err(format!("{name} 必须是 data:image/... 的 data URL"));
            }
        }
    }
    Ok(())
}

pub fn save(req: AppearanceSettings) -> Result<(), String> {
    validate(&req)?;
    let text = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    if let Some(dir) = std::path::Path::new(PATH).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(PATH, text).map_err(|e| format!("写入 {PATH} 失败: {e}"))?;
    *cache().write() = Arc::new(req.clone());
    tracing::info!(
        "外观设置已更新：style={:?} bg={} no_signal_image={}",
        req.loading_style,
        req.loading_background.as_ref().map(|v| v.len()).unwrap_or(0),
        req.no_signal_image.as_ref().map(|v| v.len()).unwrap_or(0)
    );
    Ok(())
}
