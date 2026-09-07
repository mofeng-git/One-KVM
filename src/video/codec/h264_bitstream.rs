//! H.264 Annex-B/AVCC bitstream helpers shared by WebRTC, RTSP and RustDesk.

pub const FALLBACK_WEBRTC_PROFILE_LEVEL_ID: &str = "42e01f";

pub fn webrtc_fmtp_line(profile_level_id: &str) -> String {
    format!(
        "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id={}",
        profile_level_id
    )
}

pub fn fallback_webrtc_fmtp_line() -> String {
    webrtc_fmtp_line(FALLBACK_WEBRTC_PROFILE_LEVEL_ID)
}

pub fn strip_aud_nal_units(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        let (start_code_pos, start_code_len) = if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            (i, 4)
        } else if i + 3 <= data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            (i, 3)
        } else {
            i += 1;
            continue;
        };

        let nal_start = start_code_pos + start_code_len;
        if nal_start >= data.len() {
            break;
        }

        let nal_type = data[nal_start] & 0x1F;

        let mut nal_end = data.len();
        let mut j = nal_start + 1;
        while j + 3 <= data.len() {
            if (data[j] == 0 && data[j + 1] == 0 && data[j + 2] == 1)
                || (j + 4 <= data.len()
                    && data[j] == 0
                    && data[j + 1] == 0
                    && data[j + 2] == 0
                    && data[j + 3] == 1)
            {
                nal_end = j;
                break;
            }
            j += 1;
        }

        if nal_type != 9 && nal_type != 12 {
            result.extend_from_slice(&data[start_code_pos..nal_end]);
        }

        i = nal_end;
    }

    if result.is_empty() && !data.is_empty() {
        return data.to_vec();
    }

    result
}

pub fn extract_sps_pps(data: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let mut sps: Option<Vec<u8>> = None;
    let mut pps: Option<Vec<u8>> = None;
    let mut i = 0;

    while i < data.len() {
        let start_code_len = if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            4
        } else if i + 3 <= data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            3
        } else {
            i += 1;
            continue;
        };

        let nal_start = i + start_code_len;
        if nal_start >= data.len() {
            break;
        }

        let nal_type = data[nal_start] & 0x1F;

        let mut nal_end = data.len();
        let mut j = nal_start + 1;
        while j + 3 <= data.len() {
            if (data[j] == 0 && data[j + 1] == 0 && data[j + 2] == 1)
                || (j + 4 <= data.len()
                    && data[j] == 0
                    && data[j + 1] == 0
                    && data[j + 2] == 0
                    && data[j + 3] == 1)
            {
                nal_end = j;
                break;
            }
            j += 1;
        }

        match nal_type {
            7 => {
                sps = Some(data[nal_start..nal_end].to_vec());
            }
            8 => {
                pps = Some(data[nal_start..nal_end].to_vec());
            }
            _ => {}
        }

        i = nal_end;
    }

    (sps, pps)
}

pub fn has_sps_pps(data: &[u8]) -> bool {
    let mut has_sps = false;
    let mut has_pps = false;
    let mut i = 0;

    while i < data.len() {
        let start_code_len = if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            4
        } else if i + 3 <= data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            3
        } else {
            i += 1;
            continue;
        };

        let nal_start = i + start_code_len;
        if nal_start >= data.len() {
            break;
        }

        let nal_type = data[nal_start] & 0x1F;

        match nal_type {
            7 => has_sps = true,
            8 => has_pps = true,
            _ => {}
        }

        if has_sps && has_pps {
            return true;
        }

        i = nal_start + 1;
    }

    has_sps && has_pps
}

pub fn is_keyframe(data: &[u8]) -> bool {
    let mut i = 0;
    while i < data.len() {
        if i + 3 < data.len() && data[i] == 0 && data[i + 1] == 0 {
            let nal_start = if data[i + 2] == 1 {
                i + 3
            } else if i + 4 < data.len() && data[i + 2] == 0 && data[i + 3] == 1 {
                i + 4
            } else {
                i += 1;
                continue;
            };

            if nal_start < data.len() {
                let nal_type = data[nal_start] & 0x1F;
                if nal_type == 5 {
                    return true;
                }
            }
            i = nal_start;
        } else {
            i += 1;
        }
    }
    false
}

/// `profile-level-id` hex for SDP (`42001f` etc.); expects SPS NAL without start code.
pub fn parse_profile_level_id_from_sps(sps: &[u8]) -> Option<String> {
    if sps.len() < 4 {
        return None;
    }

    let profile_idc = sps[1];
    let constraint_set_flags = sps[2];
    let level_idc = sps[3];

    Some(format!(
        "{:02x}{:02x}{:02x}",
        profile_idc, constraint_set_flags, level_idc
    ))
}

pub fn extract_profile_level_id(data: &[u8]) -> Option<String> {
    let (sps, _) = extract_sps_pps(data);
    sps.and_then(|sps_data| parse_profile_level_id_from_sps(&sps_data))
}

pub fn is_annex_b(data: &[u8]) -> bool {
    data.starts_with(&[0, 0, 1]) || data.starts_with(&[0, 0, 0, 1])
}

pub fn avcc_to_annex_b(data: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 0;
    let mut output = Vec::with_capacity(data.len() + 16);
    let mut nalu_count = 0usize;

    while pos + 4 <= data.len() {
        let nalu_len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if nalu_len == 0 || pos + nalu_len > data.len() {
            return None;
        }

        let nal_type = data[pos] & 0x1F;
        if nal_type != 9 && nal_type != 12 {
            output.extend_from_slice(&[0, 0, 0, 1]);
            output.extend_from_slice(&data[pos..pos + nalu_len]);
        }
        nalu_count += 1;
        pos += nalu_len;
    }

    if pos == data.len() && nalu_count > 0 && !output.is_empty() {
        Some(output)
    } else {
        None
    }
}

/// Normalize a length-prefixed H.264 access unit to Annex-B when necessary.
///
/// FFmpeg normally exposes elementary H.264 from hardware encoders as
/// Annex-B, but some V4L2 M2M drivers return AVCC-style packets. Consumers
/// such as RustDesk do not receive codec extradata from our protocol adapter,
/// so passing those packets through unchanged leaves the decoder unable to
/// find NAL unit boundaries.
pub fn normalize_annex_b(data: bytes::Bytes) -> bytes::Bytes {
    // A four-byte start code is unambiguous for real encoder output. A
    // three-byte prefix is not: an AVCC NAL of 256..511 bytes also begins
    // with 00 00 01. Validate AVCC before accepting that shorter prefix.
    if data.starts_with(&[0, 0, 0, 1]) {
        return data;
    }

    if let Some(annex_b) = avcc_to_annex_b(data.as_ref()) {
        return bytes::Bytes::from(annex_b);
    }

    data
}

pub fn normalize_for_webrtc(data: &[u8]) -> Vec<u8> {
    if !data.starts_with(&[0, 0, 0, 1]) {
        if let Some(annex_b) = avcc_to_annex_b(data) {
            return strip_aud_nal_units(&annex_b);
        }
    }

    if is_annex_b(data) {
        return strip_aud_nal_units(data);
    }

    data.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_h264_keyframes() {
        let idr_frame = vec![0x00, 0x00, 0x00, 0x01, 0x65];
        assert!(is_keyframe(&idr_frame));

        let idr_frame_3 = vec![0x00, 0x00, 0x01, 0x65];
        assert!(is_keyframe(&idr_frame_3));

        let p_frame = vec![0x00, 0x00, 0x00, 0x01, 0x41];
        assert!(!is_keyframe(&p_frame));

        let sps = vec![0x00, 0x00, 0x00, 0x01, 0x67];
        assert!(!is_keyframe(&sps));

        let multi_nal = vec![
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x01, 0x68, 0xce,
            0x38, 0x80, 0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84,
        ];
        assert!(is_keyframe(&multi_nal));
    }

    #[test]
    fn parses_profile_level_id_from_sps() {
        assert_eq!(
            parse_profile_level_id_from_sps(&[0x67, 0x42, 0x40, 0x2a]),
            Some("42402a".to_string())
        );
    }

    #[test]
    fn converts_avcc_access_unit_to_annex_b() {
        let avcc = [
            0, 0, 0, 4, 0x67, 0x42, 0x40, 0x1f, // SPS
            0, 0, 0, 2, 0x68, 0xce, // PPS
            0, 0, 0, 3, 0x65, 0x88, 0x84, // IDR
        ];

        let annex_b = normalize_annex_b(bytes::Bytes::copy_from_slice(&avcc));
        assert!(is_annex_b(&annex_b));
        assert!(has_sps_pps(&annex_b));
        assert!(is_keyframe(&annex_b));
    }

    #[test]
    fn leaves_annex_b_packet_unchanged() {
        let annex_b = bytes::Bytes::from_static(&[0, 0, 0, 1, 0x65, 0x88, 0x84]);
        let normalized = normalize_annex_b(annex_b.clone());
        assert_eq!(normalized, annex_b);
    }

    #[test]
    fn recognizes_avcc_length_that_looks_like_three_byte_start_code() {
        let mut avcc = vec![0, 0, 1, 0];
        avcc.push(0x65);
        avcc.resize(4 + 256, 0x88);

        let annex_b = normalize_annex_b(bytes::Bytes::from(avcc));
        assert_eq!(&annex_b[..5], &[0, 0, 0, 1, 0x65]);
        assert!(is_keyframe(&annex_b));
    }
}
