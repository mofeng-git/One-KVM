use rtsp_types as rtsp;
use std::collections::HashMap;

use super::types::RtspRequest;

pub(crate) const OPTIONS_PUBLIC_CAPABILITIES: &str =
    "OPTIONS, DESCRIBE, SETUP, PLAY, GET_PARAMETER, SET_PARAMETER, TEARDOWN";

pub(crate) fn strip_interleaved_frames_prefix(buffer: &mut Vec<u8>) -> bool {
    if buffer.len() < 4 || buffer[0] != b'$' {
        return false;
    }

    let payload_len = u16::from_be_bytes([buffer[2], buffer[3]]) as usize;
    let frame_len = 4 + payload_len;
    if buffer.len() < frame_len {
        return false;
    }

    buffer.drain(0..frame_len);
    true
}

pub(crate) fn take_rtsp_request_from_buffer(buffer: &mut Vec<u8>) -> Option<String> {
    let delimiter = b"\r\n\r\n";
    let pos = find_bytes(buffer, delimiter)?;
    let req_end = pos + delimiter.len();
    let req_bytes: Vec<u8> = buffer.drain(0..req_end).collect();
    Some(String::from_utf8_lossy(&req_bytes).to_string())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

pub(crate) fn parse_rtsp_request(raw: &str) -> Option<RtspRequest> {
    let (message, consumed): (rtsp::Message<Vec<u8>>, usize) =
        rtsp::Message::parse(raw.as_bytes()).ok()?;
    if consumed != raw.len() {
        return None;
    }

    let request = match message {
        rtsp::Message::Request(req) => req,
        _ => return None,
    };

    let uri = request
        .request_uri()
        .map(|value| value.as_str().to_string())
        .unwrap_or_default();

    let mut headers = HashMap::new();
    for (name, value) in request.headers() {
        headers.insert(name.to_string().to_ascii_lowercase(), value.to_string());
    }

    Some(RtspRequest {
        method: request.method().clone(),
        uri,
        version: request.version(),
        headers,
    })
}

pub(crate) fn parse_interleaved_channel(transport: &str) -> Option<u8> {
    let lower = transport.to_ascii_lowercase();
    if let Some((_, v)) = lower.split_once("interleaved=") {
        let head = v.split(';').next().unwrap_or(v);
        let first = head.split('-').next().unwrap_or(head).trim();
        return first.parse::<u8>().ok();
    }
    None
}

pub(crate) fn is_tcp_transport_request(transport: &str) -> bool {
    transport
        .split(',')
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .any(|item| item.contains("rtp/avp/tcp") || item.contains("interleaved="))
}

pub(crate) fn is_valid_rtsp_path(method: &rtsp::Method, uri: &str, configured_path: &str) -> bool {
    if matches!(method, rtsp::Method::Options) && uri.trim() == "*" {
        return true;
    }

    let normalized_cfg = configured_path.trim_matches('/');
    if normalized_cfg.is_empty() {
        return false;
    }

    let request_path = extract_rtsp_path(uri);

    if request_path == normalized_cfg {
        return true;
    }

    if !matches!(method, rtsp::Method::Setup | rtsp::Method::Teardown) {
        return false;
    }

    let control_track_path = format!("{}/trackID=0", normalized_cfg);
    request_path == "trackID=0" || request_path == control_track_path
}

fn extract_rtsp_path(uri: &str) -> String {
    let raw_path = if let Some((_, remainder)) = uri.split_once("://") {
        match remainder.find('/') {
            Some(idx) => &remainder[idx..],
            None => "/",
        }
    } else {
        uri
    };

    raw_path
        .split('?')
        .next()
        .unwrap_or(raw_path)
        .split('#')
        .next()
        .unwrap_or(raw_path)
        .trim_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtsp_path_matching_follows_sdp_control_rules() {
        assert!(is_valid_rtsp_path(
            &rtsp::Method::Describe,
            "rtsp://127.0.0.1/live",
            "live"
        ));
        assert!(is_valid_rtsp_path(
            &rtsp::Method::Describe,
            "rtsp://127.0.0.1/live/?token=1",
            "/live/"
        ));
        assert!(!is_valid_rtsp_path(
            &rtsp::Method::Describe,
            "rtsp://127.0.0.1/live2",
            "live"
        ));
        assert!(!is_valid_rtsp_path(
            &rtsp::Method::Describe,
            "rtsp://127.0.0.1/",
            "/"
        ));

        assert!(is_valid_rtsp_path(
            &rtsp::Method::Setup,
            "rtsp://127.0.0.1/live/trackID=0",
            "live"
        ));
        assert!(is_valid_rtsp_path(
            &rtsp::Method::Setup,
            "rtsp://127.0.0.1/trackID=0",
            "live"
        ));
        assert!(!is_valid_rtsp_path(
            &rtsp::Method::Describe,
            "rtsp://127.0.0.1/live/trackID=0",
            "live"
        ));

        assert!(is_valid_rtsp_path(&rtsp::Method::Options, "*", "live"));
    }

    #[test]
    fn transport_parsing_detects_tcp_interleaved_requests() {
        assert!(is_tcp_transport_request(
            "RTP/AVP/TCP;unicast;interleaved=0-1"
        ));
        assert!(is_tcp_transport_request("RTP/AVP;unicast;interleaved=2-3"));
        assert!(!is_tcp_transport_request(
            "RTP/AVP;unicast;client_port=8000-8001"
        ));
    }

    #[test]
    fn options_public_includes_standard_methods() {
        assert!(OPTIONS_PUBLIC_CAPABILITIES.contains("GET_PARAMETER"));
        assert!(OPTIONS_PUBLIC_CAPABILITIES.contains("TEARDOWN"));
    }
}
