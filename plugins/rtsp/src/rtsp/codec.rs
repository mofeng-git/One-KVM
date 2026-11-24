use crate::config::RtspCodec;
use one_kvm_plugin_sdk::Codec;

pub(crate) fn rtsp_codec_to_video(codec: RtspCodec) -> Codec {
    match codec {
        RtspCodec::H264 => Codec::H264,
        RtspCodec::H265 => Codec::H265,
    }
}
