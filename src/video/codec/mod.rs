//! Video codec, conversion, encoding, and decoding implementations.

use hwcodec::common::DataFormat;
use hwcodec::ffmpeg_ram::CodecInfo;

pub mod convert;

pub mod h264;
pub mod h264_bitstream;
pub mod h265;
pub mod jpeg;
pub mod registry;
pub mod self_check;
pub mod traits;
pub mod video_codec;
pub mod vp8;
pub mod vp9;

pub mod mjpeg_turbo;

#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
pub mod mjpeg_rkmpp;

pub use convert::{PixelConverter, Yuv420pBuffer};
pub use h264::{H264Config, H264Encoder, H264EncoderType, H264InputFormat};
pub use h265::{H265Config, H265Encoder, H265EncoderType, H265InputFormat};
pub use jpeg::JpegEncoder;
pub use mjpeg_turbo::MjpegTurboDecoder;
pub use registry::{AvailableEncoder, EncoderBackend, EncoderRegistry, VideoEncoderType};
pub use self_check::{
    build_hardware_self_check_runtime_error, run_hardware_self_check, VideoEncoderSelfCheckCell,
    VideoEncoderSelfCheckCodec, VideoEncoderSelfCheckResponse, VideoEncoderSelfCheckRow,
};
pub use traits::{
    BitratePreset, EncodedFormat, EncodedFrame, Encoder, EncoderConfig, EncoderFactory,
};
pub use video_codec::{
    CodecFrame, VideoCodec, VideoCodecConfig, VideoCodecFactory, VideoCodecType,
};
pub use vp8::{VP8Config, VP8Encoder, VP8EncoderType, VP8InputFormat};
pub use vp9::{VP9Config, VP9Encoder, VP9EncoderType, VP9InputFormat};

pub(crate) fn select_codec_for_format<F>(
    encoders: &[CodecInfo],
    format: DataFormat,
    preferred: F,
) -> Option<&CodecInfo>
where
    F: Fn(&CodecInfo) -> bool,
{
    encoders
        .iter()
        .find(|codec| codec.format == format && preferred(codec))
        .or_else(|| encoders.iter().find(|codec| codec.format == format))
}

pub(crate) fn detect_best_codec_for_format<T, F>(
    encoders: &[CodecInfo],
    format: DataFormat,
    preferred: F,
) -> Option<(T, String)>
where
    T: From<EncoderBackend>,
    F: Fn(&CodecInfo) -> bool,
{
    select_codec_for_format(encoders, format, preferred).map(|codec| {
        (
            T::from(EncoderBackend::from_codec_name(&codec.name)),
            codec.name.clone(),
        )
    })
}
