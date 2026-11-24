use base64::Engine;

use crate::config::RtspConfig;

use super::types::RtspRequest;

pub(crate) fn extract_basic_auth(req: &RtspRequest) -> Option<(String, String)> {
    let value = req.headers.get("authorization")?;
    let mut parts = value.split_whitespace();
    let scheme = parts.next()?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let b64 = parts.next()?;
    let decoded = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let raw = String::from_utf8(decoded).ok()?;
    let (user, pass) = raw.split_once(':')?;
    Some((user.to_string(), pass.to_string()))
}

pub(crate) fn rtsp_auth_credentials(config: &RtspConfig) -> Option<(String, String)> {
    let username = config.username.as_ref()?.trim();
    if username.is_empty() {
        return None;
    }

    Some((
        username.to_string(),
        config.password.clone().unwrap_or_default(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtsp_types as rtsp;
    use std::collections::HashMap;

    #[test]
    fn rtsp_auth_requires_non_empty_username() {
        let mut config = RtspConfig::default();
        config.password = Some("secret".to_string());
        assert!(rtsp_auth_credentials(&config).is_none());

        config.username = Some("".to_string());
        assert!(rtsp_auth_credentials(&config).is_none());

        config.username = Some("user".to_string());
        let credentials = rtsp_auth_credentials(&config).expect("expected credentials");
        assert_eq!(credentials, ("user".to_string(), "secret".to_string()));

        config.password = None;
        let credentials = rtsp_auth_credentials(&config).expect("expected credentials");
        assert_eq!(credentials, ("user".to_string(), "".to_string()));
    }

    #[test]
    fn extract_basic_auth_roundtrip() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"alice:pwd");
        let mut headers = HashMap::new();
        headers.insert("authorization".to_string(), format!("Basic {}", encoded));
        let req = RtspRequest {
            method: rtsp::Method::Options,
            uri: "*".to_string(),
            version: rtsp::Version::V1_0,
            headers,
        };
        assert_eq!(
            extract_basic_auth(&req),
            Some(("alice".to_string(), "pwd".to_string()))
        );
    }
}
