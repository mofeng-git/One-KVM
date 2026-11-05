const VPS_NAL_TYPE: u8 = 32;
const SPS_NAL_TYPE: u8 = 33;
const PPS_NAL_TYPE: u8 = 34;

fn find_start_code(data: &[u8], from: usize) -> Option<(usize, usize)> {
    let mut offset = from;
    while offset + 3 <= data.len() {
        if offset + 4 <= data.len() && data[offset..offset + 4] == [0, 0, 0, 1] {
            return Some((offset, 4));
        }
        if data[offset..offset + 3] == [0, 0, 1] {
            return Some((offset, 3));
        }
        offset += 1;
    }
    None
}

fn for_each_nal(data: &[u8], mut visit: impl FnMut(u8, &[u8])) {
    let mut cursor = 0;
    while let Some((start, start_code_len)) = find_start_code(data, cursor) {
        let nal_start = start + start_code_len;
        if nal_start + 2 > data.len() {
            break;
        }
        let next_start = find_start_code(data, nal_start)
            .map(|(offset, _)| offset)
            .unwrap_or(data.len());
        let mut nal_end = next_start;
        while nal_end > nal_start && data[nal_end - 1] == 0 {
            nal_end -= 1;
        }
        if nal_end >= nal_start + 2 {
            visit((data[nal_start] >> 1) & 0x3f, &data[nal_start..nal_end]);
        }
        if next_start == data.len() {
            break;
        }
        cursor = next_start;
    }
}

pub fn is_keyframe(data: &[u8]) -> bool {
    let mut keyframe = false;
    for_each_nal(data, |nal_type, _| {
        if (16..=23).contains(&nal_type) {
            keyframe = true;
        }
    });
    keyframe
}

pub fn extract_vps_sps_pps(data: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>) {
    let mut vps = None;
    let mut sps = None;
    let mut pps = None;
    for_each_nal(data, |nal_type, nal| match nal_type {
        VPS_NAL_TYPE => vps = Some(nal.to_vec()),
        SPS_NAL_TYPE => sps = Some(nal.to_vec()),
        PPS_NAL_TYPE => pps = Some(nal.to_vec()),
        _ => {}
    });
    (vps, sps, pps)
}

pub fn has_vps_sps_pps(data: &[u8]) -> bool {
    let (vps, sps, pps) = extract_vps_sps_pps(data);
    vps.is_some() && sps.is_some() && pps.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_irap_but_not_trail_frame() {
        assert!(is_keyframe(&[0, 0, 0, 1, 19 << 1, 1, 0xaa]));
        assert!(is_keyframe(&[0, 0, 1, 21 << 1, 1, 0xbb]));
        assert!(!is_keyframe(&[0, 0, 0, 1, 1 << 1, 1, 0xcc]));
    }

    #[test]
    fn extracts_parameter_sets() {
        let data = [
            0,
            0,
            0,
            1,
            32 << 1,
            1,
            0xaa,
            0,
            0,
            1,
            33 << 1,
            1,
            0xbb,
            0,
            0,
            0,
            1,
            34 << 1,
            1,
            0xcc,
        ];
        let (vps, sps, pps) = extract_vps_sps_pps(&data);
        assert_eq!(vps.unwrap(), [32 << 1, 1, 0xaa]);
        assert_eq!(sps.unwrap(), [33 << 1, 1, 0xbb]);
        assert_eq!(pps.unwrap(), [34 << 1, 1, 0xcc]);
        assert!(has_vps_sps_pps(&data));
    }
}
