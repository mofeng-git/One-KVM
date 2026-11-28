use axum::{extract::State, Json};
use openssl::asn1::{Asn1Time, Asn1TimeRef};
use openssl::pkey::PKey;
use openssl::x509::X509NameRef;
use std::net::IpAddr;
use std::sync::Arc;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::config::WebConfig;
use crate::error::{AppError, Result};
use crate::state::AppState;
use crate::utils::tls::{certificate_chain_from_pem, server_config_from_pem};

use super::types::{WebCertificateInfo, WebConfigResponse, WebConfigUpdate};

pub async fn get_web_config(State(state): State<Arc<AppState>>) -> Json<WebConfigResponse> {
    Json(WebConfigResponse::from_stored(&state.config.get().web).await)
}

pub async fn update_web_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WebConfigUpdate>,
) -> Result<Json<WebConfigResponse>> {
    req.validate()?;

    // Determine certificate path changes (requires async file I/O before config update)
    // Some(Some((cert, key))) = write new cert
    // Some(None)              = clear custom cert
    // None                    = no cert change
    let cert_path_update: Option<Option<(String, String)>> = if let (
        Some(cert_pem),
        Some(key_pem),
    ) =
        (&req.ssl_cert_pem, &req.ssl_key_pem)
    {
        server_config_from_pem(cert_pem.as_bytes(), key_pem.as_bytes())
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

    Ok(Json(
        WebConfigResponse::from_stored(&state.config.get().web).await,
    ))
}

impl WebConfigResponse {
    async fn from_stored(web: &WebConfig) -> Self {
        let cert_paths = web
            .ssl_cert_path
            .as_deref()
            .zip(web.ssl_key_path.as_deref())
            .filter(|(cert, key)| !cert.is_empty() && !key.is_empty());
        let custom_cert_info = match cert_paths {
            Some((cert_path, key_path)) => read_certificate_info(cert_path, key_path).await,
            None => None,
        };
        Self {
            http_port: web.http_port,
            https_port: web.https_port,
            bind_addresses: web.bind_addresses.clone(),
            bind_address: web.bind_address.clone(),
            https_enabled: web.https_enabled,
            has_custom_cert: cert_paths.is_some(),
            custom_cert_info,
        }
    }
}

async fn read_certificate_info(cert_path: &str, key_path: &str) -> Option<WebCertificateInfo> {
    let cert_pem = tokio::fs::read(cert_path).await.ok()?;
    let (certificate, _) = certificate_chain_from_pem(&cert_pem).ok()?;
    let key_pem = tokio::fs::read(key_path).await.ok();
    let key_matches = key_pem.and_then(|key_pem| {
        // Avoid a passphrase prompt when inspecting a saved key.
        let key = PKey::private_key_from_pem_passphrase(&key_pem, b"").ok()?;
        Some(certificate.public_key().ok()?.public_eq(&key))
    });

    let names = certificate
        .subject_alt_names()
        .iter()
        .flatten()
        .filter_map(|name| {
            name.dnsname().map(str::to_owned).or_else(|| {
                let ip = match name.ipaddress()? {
                    [a, b, c, d] => IpAddr::from([*a, *b, *c, *d]),
                    bytes => IpAddr::from(<[u8; 16]>::try_from(bytes).ok()?),
                };
                Some(ip.to_string())
            })
        })
        .collect();

    Some(WebCertificateInfo {
        subject: format_certificate_name(certificate.subject_name()),
        issuer: format_certificate_name(certificate.issuer_name()),
        not_before: format_certificate_time(certificate.not_before())?,
        not_after: format_certificate_time(certificate.not_after())?,
        names,
        key_matches,
    })
}

fn format_certificate_name(name: &X509NameRef) -> String {
    name.entries()
        .filter_map(|entry| {
            let value = entry.data().to_string().ok()?;
            let object = entry.object();
            let field = object
                .nid()
                .short_name()
                .map(str::to_owned)
                .unwrap_or_else(|_| object.to_string());
            Some(format!("{field}={value}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_certificate_time(time: &Asn1TimeRef) -> Option<String> {
    let epoch = Asn1Time::from_unix(0).ok()?;
    let offset = epoch.diff(time).ok()?;
    let timestamp = i64::from(offset.days) * 86_400 + i64::from(offset.secs);
    OffsetDateTime::from_unix_timestamp(timestamp)
        .ok()?
        .format(&Rfc3339)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reports_saved_certificate_details_and_key_status() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("custom.crt");
        let key_path = dir.path().join("custom.key");
        let mut params = rcgen::CertificateParams::new(vec![
            "*.example.test".into(),
            "192.0.2.10".into(),
            "2001:db8::1".into(),
        ])
        .unwrap();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "example.test");
        params.not_before = rcgen::date_time_ymd(2020, 2, 29);
        params.not_after = rcgen::date_time_ymd(2051, 3, 1);
        let key = rcgen::KeyPair::generate().unwrap();
        let certificate = params.self_signed(&key).unwrap();
        let chain_tail = rcgen::generate_simple_self_signed(vec!["issuer.test".into()]).unwrap();
        tokio::fs::write(
            &cert_path,
            format!("{}{}", certificate.pem(), chain_tail.cert.pem()),
        )
        .await
        .unwrap();
        tokio::fs::write(&key_path, key.serialize_pem())
            .await
            .unwrap();

        let web = WebConfig {
            ssl_cert_path: Some(cert_path.to_string_lossy().into_owned()),
            ssl_key_path: Some(key_path.to_string_lossy().into_owned()),
            ..WebConfig::default()
        };
        let config = WebConfigResponse::from_stored(&web).await;
        assert!(config.has_custom_cert);
        let info = config.custom_cert_info.unwrap();
        assert_eq!(info.subject, "CN=example.test");
        assert_eq!(info.issuer, "CN=example.test");
        assert_eq!(info.not_before, "2020-02-29T00:00:00Z");
        assert_eq!(info.not_after, "2051-03-01T00:00:00Z");
        assert_eq!(info.names, ["*.example.test", "192.0.2.10", "2001:db8::1"]);
        assert_eq!(info.key_matches, Some(true));

        tokio::fs::write(&key_path, chain_tail.signing_key.serialize_pem())
            .await
            .unwrap();
        let config = WebConfigResponse::from_stored(&web).await;
        assert_eq!(config.custom_cert_info.unwrap().key_matches, Some(false));

        tokio::fs::remove_file(&key_path).await.unwrap();
        let config = WebConfigResponse::from_stored(&web).await;
        assert_eq!(config.custom_cert_info.unwrap().key_matches, None);

        tokio::fs::write(&cert_path, b"invalid certificate")
            .await
            .unwrap();
        let config = WebConfigResponse::from_stored(&web).await;
        assert!(config.has_custom_cert);
        assert!(config.custom_cert_info.is_none());

        let config = WebConfigResponse::from_stored(&WebConfig::default()).await;
        assert!(!config.has_custom_cert);
        assert!(config.custom_cert_info.is_none());
    }

    #[test]
    fn openssl_accepts_rcgen_self_signed_pem() {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_pem = cert.cert.pem();
        let key_pem = cert.signing_key.serialize_pem();
        server_config_from_pem(cert_pem.as_bytes(), key_pem.as_bytes()).unwrap();
    }
}
