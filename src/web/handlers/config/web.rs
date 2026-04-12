//! Web 服务器配置 Handler

use axum::{extract::State, Json};
use axum_server::tls_rustls::RustlsConfig;
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::state::AppState;

use super::types::{WebConfigResponse, WebConfigUpdate};

/// 获取 Web 配置
pub async fn get_web_config(
    State(state): State<Arc<AppState>>,
) -> Json<WebConfigResponse> {
    Json(WebConfigResponse::from_stored(&state.config.get().web))
}

/// 更新 Web 配置（支持 PEM 证书上传）
pub async fn update_web_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WebConfigUpdate>,
) -> Result<Json<WebConfigResponse>> {
    req.validate()?;

    // Determine certificate path changes (requires async file I/O before config update)
    // Some(Some((cert, key))) = write new cert
    // Some(None)              = clear custom cert
    // None                    = no cert change
    let cert_path_update: Option<Option<(String, String)>> =
        if let (Some(cert_pem), Some(key_pem)) = (&req.ssl_cert_pem, &req.ssl_key_pem) {
            RustlsConfig::from_pem(cert_pem.as_bytes().to_vec(), key_pem.as_bytes().to_vec())
                .await
                .map_err(|e| {
                    AppError::BadRequest(
                        format!(
                            "Invalid TLS certificate or private key (PEM must match what the HTTPS server can load): {e}"
                        )
                        .into(),
                    )
                })?;
            let cert_dir = state.data_dir().join("certs");
            tokio::fs::create_dir_all(&cert_dir)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to create cert dir: {e}")))?;
            let cert_path = cert_dir.join("custom.crt");
            let key_path = cert_dir.join("custom.key");
            tokio::fs::write(&cert_path, cert_pem.as_bytes())
                .await
                .map_err(|e| AppError::Internal(format!("Failed to write certificate: {e}")))?;
            tokio::fs::write(&key_path, key_pem.as_bytes())
                .await
                .map_err(|e| AppError::Internal(format!("Failed to write private key: {e}")))?;
            Some(Some((
                cert_path.to_string_lossy().into_owned(),
                key_path.to_string_lossy().into_owned(),
            )))
        } else if req.clear_custom_cert.unwrap_or(false) {
            let cert_dir = state.data_dir().join("certs");
            let _ = tokio::fs::remove_file(cert_dir.join("custom.crt")).await;
            let _ = tokio::fs::remove_file(cert_dir.join("custom.key")).await;
            Some(None)
        } else {
            None
        };

    state
        .config
        .update(move |config| {
            req.apply_to(&mut config.web);
            match cert_path_update {
                Some(Some((cert_path, key_path))) => {
                    config.web.ssl_cert_path = Some(cert_path);
                    config.web.ssl_key_path = Some(key_path);
                }
                Some(None) => {
                    config.web.ssl_cert_path = None;
                    config.web.ssl_key_path = None;
                }
                None => {}
            }
        })
        .await?;

    Ok(Json(WebConfigResponse::from_stored(&state.config.get().web)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::crypto::{ring, CryptoProvider};

    #[tokio::test]
    async fn rustls_accepts_rcgen_self_signed_pem() {
        let _ = CryptoProvider::install_default(ring::default_provider());
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_pem = cert.cert.pem();
        let key_pem = cert.signing_key.serialize_pem();
        RustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes())
            .await
            .unwrap();
    }
}
