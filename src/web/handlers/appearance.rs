//! 外观设置的 HTTP 接口：GET/PUT /api/appearance

use axum::http::StatusCode;
use axum::Json;

pub async fn get_appearance() -> Json<crate::appearance::AppearanceSettings> {
    Json((*crate::appearance::current()).clone())
}

/// 按当前设置读一次链路寄存器，供设置页"读取一次"用
pub async fn read_signal() -> Json<serde_json::Value> {
    match crate::video::signal_probe::read_once() {
        Ok((node, raw)) => Json(serde_json::json!({"ok": true, "node": node, "raw": raw})),
        Err(error) => Json(serde_json::json!({"ok": false, "error": error})),
    }
}

pub async fn put_appearance(
    Json(req): Json<crate::appearance::AppearanceSettings>,
) -> Result<Json<crate::appearance::AppearanceSettings>, (StatusCode, String)> {
    crate::appearance::save(req.clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(req))
}
