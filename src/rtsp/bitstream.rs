use bytes::Bytes;

use crate::video::encoder::registry::VideoEncoderType;
use crate::video::shared_video_pipeline::EncodedVideoFrame;

use super::state::ParameterSets;

pub(crate) fn update_parameter_sets(params: &mut ParameterSets, frame: &EncodedVideoFrame) {
    let nal_units = split_annexb_nal_units(frame.data.as_ref());

    match frame.codec {
        VideoEncoderType::H264 => {
            for nal in nal_units {
                match h264_nal_type(nal) {
                    Some(7) => params.h264_sps = Some(Bytes::copy_from_slice(nal)),
                    Some(8) => params.h264_pps = Some(Bytes::copy_from_slice(nal)),
                    _ => {}
                }
            }
        }
        VideoEncoderType::H265 => {
            for nal in nal_units {
                match h265_nal_type(nal) {
                    Some(32) => params.h265_vps = Some(Bytes::copy_from_slice(nal)),
                    Some(33) => params.h265_sps = Some(Bytes::copy_from_slice(nal)),
                    Some(34) => params.h265_pps = Some(Bytes::copy_from_slice(nal)),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn split_annexb_nal_units(data: &[u8]) -> Vec<&[u8]> {
    let mut nal_units = Vec::new();
    let mut cursor = 0usize;

    while let Some((start, start_code_len)) = find_annexb_start_code(data, cursor) {
        let nal_start = start + start_code_len;
        if nal_start >= data.len() {
            break;
        }

        let next_start = find_annexb_start_code(data, nal_start)
            .map(|(idx, _)| idx)
            .unwrap_or(data.len());

        let mut nal_end = next_start;
        while nal_end > nal_start && data[nal_end - 1] == 0 {
            nal_end -= 1;
        }

        if nal_end > nal_start {
            nal_units.push(&data[nal_start..nal_end]);
        }

        cursor = next_start;
    }

    nal_units
}

fn find_annexb_start_code(data: &[u8], from: usize) -> Option<(usize, usize)> {
    if from >= data.len() {
        return None;
    }

    let mut i = from;
    while i + 3 <= data.len() {
        if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            return Some((i, 4));
        }

        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            return Some((i, 3));
        }

        i += 1;
    }

    None
}

fn h264_nal_type(nal: &[u8]) -> Option<u8> {
    nal.first().map(|value| value & 0x1f)
}

fn h265_nal_type(nal: &[u8]) -> Option<u8> {
    nal.first().map(|value| (value >> 1) & 0x3f)
}
