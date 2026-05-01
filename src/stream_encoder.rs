//! `EncoderType` → `EncoderBackend` (breaks config ↔ video import cycles).

use crate::config::EncoderType;
use crate::video::encoder::EncoderBackend;

/// `None` means “auto” in WebRTC / pipeline (same as `EncoderType::Auto`).
pub fn encoder_type_to_backend(encoder: EncoderType) -> Option<EncoderBackend> {
    match encoder {
        EncoderType::Auto => None,
        EncoderType::Software => Some(EncoderBackend::Software),
        EncoderType::Vaapi => Some(EncoderBackend::Vaapi),
        EncoderType::Nvenc => Some(EncoderBackend::Nvenc),
        EncoderType::Qsv => Some(EncoderBackend::Qsv),
        EncoderType::Amf => Some(EncoderBackend::Amf),
        EncoderType::Rkmpp => Some(EncoderBackend::Rkmpp),
        EncoderType::V4l2m2m => Some(EncoderBackend::V4l2m2m),
    }
}
