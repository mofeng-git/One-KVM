//! 外观设置的 HTTP 接口：GET/PUT /api/appearance

use axum::http::StatusCode;
use axum::Json;

pub async fn get_appearance() -> Json<crate::appearance::AppearanceSettings> {
    Json((*crate::appearance::current()).clone())
}

pub async fn put_appearance(
    Json(req): Json<crate::appearance::AppearanceSettings>,
) -> Result<Json<crate::appearance::AppearanceSettings>, (StatusCode, String)> {
    crate::appearance::save(req.clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(req))
}
