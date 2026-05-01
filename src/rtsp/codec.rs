use crate::config::RtspCodec;
use crate::video::encoder::VideoCodecType;

pub(crate) fn rtsp_codec_to_video(codec: RtspCodec) -> VideoCodecType {
    match codec {
        RtspCodec::H264 => VideoCodecType::H264,
        RtspCodec::H265 => VideoCodecType::H265,
    }
}
