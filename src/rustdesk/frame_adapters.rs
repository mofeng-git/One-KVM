//! RustDesk Frame Adapters
//!
//! Converts One-KVM video/audio frames to RustDesk protocol format.
//! Optimized for zero-copy where possible and buffer reuse.

use bytes::Bytes;
use protobuf::Message as ProtobufMessage;

use super::protocol::hbb::message::{
    message as msg_union, misc as misc_union, video_frame as vf_union, AudioFormat, AudioFrame,
    CursorData, CursorPosition, EncodedVideoFrame, EncodedVideoFrames, Message, Misc, VideoFrame,
};

/// Video codec type for RustDesk
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
    VP8,
    VP9,
    AV1,
}

impl VideoCodec {
    /// Get the codec ID for the RustDesk protocol
    pub fn to_codec_id(self) -> i32 {
        match self {
            VideoCodec::H264 => 0,
            VideoCodec::H265 => 1,
            VideoCodec::VP8 => 2,
            VideoCodec::VP9 => 3,
            VideoCodec::AV1 => 4,
        }
    }
}

/// Video frame adapter for converting to RustDesk format
pub struct VideoFrameAdapter {
    /// Current codec
    codec: VideoCodec,
    /// Frame sequence number
    seq: u32,
    /// Timestamp offset
    timestamp_base: u64,
    /// Cached H264 SPS/PPS (Annex B NAL without start code)
    h264_sps: Option<Bytes>,
    h264_pps: Option<Bytes>,
}

impl VideoFrameAdapter {
    /// Create a new video frame adapter
    pub fn new(codec: VideoCodec) -> Self {
        Self {
            codec,
            seq: 0,
            timestamp_base: 0,
            h264_sps: None,
            h264_pps: None,
        }
    }

    /// Set codec type
    pub fn set_codec(&mut self, codec: VideoCodec) {
        self.codec = codec;
    }

    /// Convert encoded video data to RustDesk Message (zero-copy version)
    ///
    /// This version takes Bytes directly to avoid copying the frame data.
    pub fn encode_frame_from_bytes(
        &mut self,
        data: Bytes,
        is_keyframe: bool,
        timestamp_ms: u64,
    ) -> Message {
        let data = self.prepare_h264_frame(data, is_keyframe);
        // Calculate relative timestamp
        if self.seq == 0 {
            self.timestamp_base = timestamp_ms;
        }
        let pts = (timestamp_ms - self.timestamp_base) as i64;

        let mut frame = EncodedVideoFrame::new();
        frame.data = data;
        frame.key = is_keyframe;
        frame.pts = pts;

        self.seq = self.seq.wrapping_add(1);

        // Wrap in EncodedVideoFrames container
        let mut frames = EncodedVideoFrames::new();
        frames.frames.push(frame);

        // Create the appropriate VideoFrame variant based on codec
        let mut video_frame = VideoFrame::new();
        match self.codec {
            VideoCodec::H264 => video_frame.union = Some(vf_union::Union::H264s(frames)),
            VideoCodec::H265 => video_frame.union = Some(vf_union::Union::H265s(frames)),
            VideoCodec::VP8 => video_frame.union = Some(vf_union::Union::Vp8s(frames)),
            VideoCodec::VP9 => video_frame.union = Some(vf_union::Union::Vp9s(frames)),
            VideoCodec::AV1 => video_frame.union = Some(vf_union::Union::Av1s(frames)),
        }

        let mut msg = Message::new();
        msg.union = Some(msg_union::Union::VideoFrame(video_frame));
        msg
    }

    fn prepare_h264_frame(&mut self, data: Bytes, is_keyframe: bool) -> Bytes {
        if self.codec != VideoCodec::H264 {
            return data;
        }

        // Parse SPS/PPS from Annex B data (without start codes)
        let (sps, pps) = crate::webrtc::rtp::extract_sps_pps(&data);
        let mut has_sps = false;
        let mut has_pps = false;

        if let Some(sps) = sps {
            self.h264_sps = Some(Bytes::from(sps));
            has_sps = true;
        }
        if let Some(pps) = pps {
            self.h264_pps = Some(Bytes::from(pps));
            has_pps = true;
        }

        // Inject cached SPS/PPS before IDR when missing
        if is_keyframe && (!has_sps || !has_pps) {
            if let (Some(sps), Some(pps)) = (self.h264_sps.as_ref(), self.h264_pps.as_ref())
            {
                let mut out = Vec::with_capacity(8 + sps.len() + pps.len() + data.len());
                out.extend_from_slice(&[0, 0, 0, 1]);
                out.extend_from_slice(sps);
                out.extend_from_slice(&[0, 0, 0, 1]);
                out.extend_from_slice(pps);
                out.extend_from_slice(&data);
                return Bytes::from(out);
            }
        }

        data
    }

    /// Convert encoded video data to RustDesk Message
    pub fn encode_frame(&mut self, data: &[u8], is_keyframe: bool, timestamp_ms: u64) -> Message {
        self.encode_frame_from_bytes(Bytes::copy_from_slice(data), is_keyframe, timestamp_ms)
    }

    /// Encode frame to bytes for sending (zero-copy version)
    ///
    /// Takes Bytes directly to avoid copying the frame data.
    pub fn encode_frame_bytes_zero_copy(
        &mut self,
        data: Bytes,
        is_keyframe: bool,
        timestamp_ms: u64,
    ) -> Bytes {
        let msg = self.encode_frame_from_bytes(data, is_keyframe, timestamp_ms);
        Bytes::from(msg.write_to_bytes().unwrap_or_default())
    }

    /// Encode frame to bytes for sending
    pub fn encode_frame_bytes(
        &mut self,
        data: &[u8],
        is_keyframe: bool,
        timestamp_ms: u64,
    ) -> Bytes {
        self.encode_frame_bytes_zero_copy(Bytes::copy_from_slice(data), is_keyframe, timestamp_ms)
    }

    /// Get current sequence number
    pub fn seq(&self) -> u32 {
        self.seq
    }
}

/// Audio frame adapter for converting to RustDesk format
pub struct AudioFrameAdapter {
    /// Sample rate
    sample_rate: u32,
    /// Channels
    channels: u8,
    /// Format sent flag
    format_sent: bool,
}

impl AudioFrameAdapter {
    /// Create a new audio frame adapter
    pub fn new(sample_rate: u32, channels: u8) -> Self {
        Self {
            sample_rate,
            channels,
            format_sent: false,
        }
    }

    /// Create audio format message (should be sent once before audio frames)
    pub fn create_format_message(&mut self) -> Message {
        self.format_sent = true;

        let mut format = AudioFormat::new();
        format.sample_rate = self.sample_rate;
        format.channels = self.channels as u32;

        let mut misc = Misc::new();
        misc.union = Some(misc_union::Union::AudioFormat(format));

        let mut msg = Message::new();
        msg.union = Some(msg_union::Union::Misc(misc));
        msg
    }

    /// Check if format message has been sent
    pub fn format_sent(&self) -> bool {
        self.format_sent
    }

    /// Convert Opus audio data to RustDesk Message
    pub fn encode_opus_frame(&self, data: &[u8]) -> Message {
        let mut frame = AudioFrame::new();
        frame.data = Bytes::copy_from_slice(data);

        let mut msg = Message::new();
        msg.union = Some(msg_union::Union::AudioFrame(frame));
        msg
    }

    /// Encode Opus frame to bytes for sending
    pub fn encode_opus_bytes(&self, data: &[u8]) -> Bytes {
        let msg = self.encode_opus_frame(data);
        Bytes::from(msg.write_to_bytes().unwrap_or_default())
    }

    /// Reset state (call when restarting audio stream)
    pub fn reset(&mut self) {
        self.format_sent = false;
    }
}

/// Cursor data adapter
pub struct CursorAdapter;

impl CursorAdapter {
    /// Create cursor data message
    pub fn encode_cursor(
        id: u64,
        hotx: i32,
        hoty: i32,
        width: i32,
        height: i32,
        colors: Vec<u8>,
    ) -> Message {
        let mut cursor = CursorData::new();
        cursor.id = id;
        cursor.hotx = hotx;
        cursor.hoty = hoty;
        cursor.width = width;
        cursor.height = height;
        cursor.colors = Bytes::from(colors);

        let mut msg = Message::new();
        msg.union = Some(msg_union::Union::CursorData(cursor));
        msg
    }

    /// Create cursor position message
    pub fn encode_position(x: i32, y: i32) -> Message {
        let mut pos = CursorPosition::new();
        pos.x = x;
        pos.y = y;

        let mut msg = Message::new();
        msg.union = Some(msg_union::Union::CursorPosition(pos));
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_frame_encoding() {
        let mut adapter = VideoFrameAdapter::new(VideoCodec::H264);

        // Encode a keyframe
        let data = vec![0x00, 0x00, 0x00, 0x01, 0x67]; // H264 SPS NAL
        let msg = adapter.encode_frame(&data, true, 0);

        match &msg.union {
            Some(msg_union::Union::VideoFrame(vf)) => match &vf.union {
                Some(vf_union::Union::H264s(frames)) => {
                    assert_eq!(frames.frames.len(), 1);
                    assert!(frames.frames[0].key);
                }
                _ => panic!("Expected H264s"),
            },
            _ => panic!("Expected VideoFrame"),
        }
    }

    #[test]
    fn test_audio_format_message() {
        let mut adapter = AudioFrameAdapter::new(48000, 2);
        assert!(!adapter.format_sent());

        let msg = adapter.create_format_message();
        assert!(adapter.format_sent());

        match &msg.union {
            Some(msg_union::Union::Misc(misc)) => match &misc.union {
                Some(misc_union::Union::AudioFormat(fmt)) => {
                    assert_eq!(fmt.sample_rate, 48000);
                    assert_eq!(fmt.channels, 2);
                }
                _ => panic!("Expected AudioFormat"),
            },
            _ => panic!("Expected Misc"),
        }
    }

    #[test]
    fn test_audio_frame_encoding() {
        let adapter = AudioFrameAdapter::new(48000, 2);

        // Encode an Opus frame
        let opus_data = vec![0xFC, 0x01, 0x02]; // Fake Opus data
        let msg = adapter.encode_opus_frame(&opus_data);

        match &msg.union {
            Some(msg_union::Union::AudioFrame(af)) => {
                assert_eq!(&af.data[..], &opus_data[..]);
            }
            _ => panic!("Expected AudioFrame"),
        }
    }

    #[test]
    fn test_cursor_encoding() {
        let msg = CursorAdapter::encode_cursor(1, 0, 0, 16, 16, vec![0xFF; 16 * 16 * 4]);

        match &msg.union {
            Some(msg_union::Union::CursorData(cd)) => {
                assert_eq!(cd.id, 1);
                assert_eq!(cd.width, 16);
                assert_eq!(cd.height, 16);
            }
            _ => panic!("Expected CursorData"),
        }
    }

    #[test]
    fn test_sequence_increment() {
        let mut adapter = VideoFrameAdapter::new(VideoCodec::H264);

        assert_eq!(adapter.seq(), 0);
        adapter.encode_frame(&[0], false, 0);
        assert_eq!(adapter.seq(), 1);
        adapter.encode_frame(&[0], false, 33);
        assert_eq!(adapter.seq(), 2);
    }
}
