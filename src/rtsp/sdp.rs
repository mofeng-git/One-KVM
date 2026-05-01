use base64::Engine;
use sdp_types as sdp;

use crate::config::RtspConfig;
use crate::video::encoder::VideoCodecType;
use crate::webrtc::rtp::parse_profile_level_id_from_sps;

use super::state::ParameterSets;

pub(crate) fn build_h264_fmtp(payload_type: u8, params: &ParameterSets) -> String {
    let mut attrs = vec!["packetization-mode=1".to_string()];

    if let Some(sps) = params.h264_sps.as_ref() {
        if let Some(profile_level_id) = parse_profile_level_id_from_sps(sps) {
            attrs.push(format!("profile-level-id={}", profile_level_id));
        }
    } else {
        attrs.push("profile-level-id=42e01f".to_string());
    }

    if let (Some(sps), Some(pps)) = (params.h264_sps.as_ref(), params.h264_pps.as_ref()) {
        let sps_b64 = base64::engine::general_purpose::STANDARD.encode(sps.as_ref());
        let pps_b64 = base64::engine::general_purpose::STANDARD.encode(pps.as_ref());
        attrs.push(format!("sprop-parameter-sets={},{}", sps_b64, pps_b64));
    }

    format!("{} {}", payload_type, attrs.join(";"))
}

pub(crate) fn build_h265_fmtp(payload_type: u8, params: &ParameterSets) -> String {
    let mut attrs = Vec::new();

    if let Some(vps) = params.h265_vps.as_ref() {
        attrs.push(format!(
            "sprop-vps={}",
            base64::engine::general_purpose::STANDARD.encode(vps.as_ref())
        ));
    }

    if let Some(sps) = params.h265_sps.as_ref() {
        attrs.push(format!(
            "sprop-sps={}",
            base64::engine::general_purpose::STANDARD.encode(sps.as_ref())
        ));
    }

    if let Some(pps) = params.h265_pps.as_ref() {
        attrs.push(format!(
            "sprop-pps={}",
            base64::engine::general_purpose::STANDARD.encode(pps.as_ref())
        ));
    }

    if attrs.is_empty() {
        format!("{} profile-id=1", payload_type)
    } else {
        format!("{} {}", payload_type, attrs.join(";"))
    }
}

pub(crate) fn build_sdp(
    config: &RtspConfig,
    codec: VideoCodecType,
    params: &ParameterSets,
) -> String {
    let (payload_type, codec_name, fmtp_value) = match codec {
        VideoCodecType::H264 => (96u8, "H264", build_h264_fmtp(96, params)),
        VideoCodecType::H265 => (99u8, "H265", build_h265_fmtp(99, params)),
        _ => {
            tracing::warn!("RTSP SDP: unexpected VideoCodecType, falling back to H264");
            (96u8, "H264", build_h264_fmtp(96, params))
        }
    };

    let session = sdp::Session {
        origin: sdp::Origin {
            username: Some("-".to_string()),
            sess_id: "0".to_string(),
            sess_version: 0,
            nettype: "IN".to_string(),
            addrtype: "IP4".to_string(),
            unicast_address: config.bind.clone(),
        },
        session_name: "One-KVM RTSP Stream".to_string(),
        session_description: None,
        uri: None,
        emails: Vec::new(),
        phones: Vec::new(),
        connection: Some(sdp::Connection {
            nettype: "IN".to_string(),
            addrtype: "IP4".to_string(),
            connection_address: "0.0.0.0".to_string(),
        }),
        bandwidths: Vec::new(),
        times: vec![sdp::Time {
            start_time: 0,
            stop_time: 0,
            repeats: Vec::new(),
        }],
        time_zones: Vec::new(),
        key: None,
        attributes: vec![sdp::Attribute {
            attribute: "control".to_string(),
            value: Some("*".to_string()),
        }],
        medias: vec![sdp::Media {
            media: "video".to_string(),
            port: 0,
            num_ports: None,
            proto: "RTP/AVP".to_string(),
            fmt: payload_type.to_string(),
            media_title: None,
            connections: Vec::new(),
            bandwidths: Vec::new(),
            key: None,
            attributes: vec![
                sdp::Attribute {
                    attribute: "rtpmap".to_string(),
                    value: Some(format!("{} {}/90000", payload_type, codec_name)),
                },
                sdp::Attribute {
                    attribute: "fmtp".to_string(),
                    value: Some(fmtp_value),
                },
                sdp::Attribute {
                    attribute: "control".to_string(),
                    value: Some("trackID=0".to_string()),
                },
            ],
        }],
    };

    let mut output = Vec::new();
    if let Err(err) = session.write(&mut output) {
        tracing::warn!("Failed to serialize SDP with sdp-types: {}", err);
        return String::new();
    }

    match String::from_utf8(output) {
        Ok(sdp_text) => sdp_text,
        Err(err) => {
            tracing::warn!("Failed to convert SDP bytes to UTF-8: {}", err);
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RtspConfig;
    use bytes::Bytes;

    #[test]
    fn build_sdp_h264_is_parseable_with_expected_video_attributes() {
        let config = RtspConfig::default();
        let mut params = ParameterSets::default();
        params.h264_sps = Some(Bytes::from_static(&[0x67, 0x42, 0xe0, 0x1f, 0x96, 0x54]));
        params.h264_pps = Some(Bytes::from_static(&[0x68, 0xce, 0x06, 0xe2]));

        let sdp_text = build_sdp(&config, VideoCodecType::H264, &params);
        assert!(!sdp_text.is_empty());

        let session = sdp::Session::parse(sdp_text.as_bytes()).expect("sdp parse failed");
        assert_eq!(session.session_name, "One-KVM RTSP Stream");
        assert_eq!(session.medias.len(), 1);

        let media = &session.medias[0];
        assert_eq!(media.media, "video");
        assert_eq!(media.proto, "RTP/AVP");
        assert_eq!(media.fmt, "96");

        let has_rtpmap = media.attributes.iter().any(|attr| {
            attr.attribute == "rtpmap" && attr.value.as_deref() == Some("96 H264/90000")
        });
        assert!(has_rtpmap);

        let fmtp_value = media
            .attributes
            .iter()
            .find(|attr| attr.attribute == "fmtp")
            .and_then(|attr| attr.value.as_deref())
            .expect("missing fmtp value");
        assert!(fmtp_value.starts_with("96 "));
        assert!(fmtp_value.contains("packetization-mode=1"));
        assert!(fmtp_value.contains("sprop-parameter-sets="));
    }

    #[test]
    fn build_sdp_h265_is_parseable_with_expected_video_attributes() {
        let config = RtspConfig::default();
        let mut params = ParameterSets::default();
        params.h265_vps = Some(Bytes::from_static(&[0x40, 0x01, 0x0c, 0x01]));
        params.h265_sps = Some(Bytes::from_static(&[0x42, 0x01, 0x01, 0x60]));
        params.h265_pps = Some(Bytes::from_static(&[0x44, 0x01, 0xc0, 0x73]));

        let sdp_text = build_sdp(&config, VideoCodecType::H265, &params);
        assert!(!sdp_text.is_empty());

        let session = sdp::Session::parse(sdp_text.as_bytes()).expect("sdp parse failed");
        assert_eq!(session.medias.len(), 1);

        let media = &session.medias[0];
        assert_eq!(media.media, "video");
        assert_eq!(media.proto, "RTP/AVP");
        assert_eq!(media.fmt, "99");

        let has_rtpmap = media.attributes.iter().any(|attr| {
            attr.attribute == "rtpmap" && attr.value.as_deref() == Some("99 H265/90000")
        });
        assert!(has_rtpmap);

        let fmtp_value = media
            .attributes
            .iter()
            .find(|attr| attr.attribute == "fmtp")
            .and_then(|attr| attr.value.as_deref())
            .expect("missing fmtp value");
        assert!(fmtp_value.starts_with("99 "));
        assert!(fmtp_value.contains("sprop-vps="));
        assert!(fmtp_value.contains("sprop-sps="));
        assert!(fmtp_value.contains("sprop-pps="));
    }
}
