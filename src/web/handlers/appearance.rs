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

/// 连续读 10 次，返回取值分布，用来判断寄存器是否稳定
pub async fn sample_signal() -> Json<serde_json::Value> {
    match crate::video::signal_probe::read_sample(10) {
        Ok((node, values)) => {
            let mut counts: Vec<(u8, usize)> = Vec::new();
            for value in &values {
                match counts.iter_mut().find(|(v, _)| v == value) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((*value, 1)),
                }
            }
            Json(serde_json::json!({
                "ok": true,
                "node": node,
                "values": values,
                "distinct": counts.iter().map(|(v, n)| serde_json::json!({"value": v, "count": n})).collect::<Vec<_>>(),
            }))
        }
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
