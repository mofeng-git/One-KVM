//! WebRTC module for low-latency video streaming
//!
//! This module provides WebRTC-based video streaming with:
//! - H.264 video track (hardware/software encoding)
//! - H.265 video track (hardware only)
//! - VP8/VP9 video track (hardware only - VAAPI)
//! - Opus audio track (optional)
//! - DataChannel for HID events
//!
//! Architecture:
//! ```text
//! VideoCapturer (MJPEG/YUYV)
//!        |
//!        v
//! SharedVideoPipeline (decode -> convert -> encode)
//!        |
//!        v
//! UniversalVideoTrack (RTP packetization)
//!        |
//!        v
//! WebRTC PeerConnection
//!        |
//! Browser <-------- SDP Exchange ------- API Server
//!        |
//!        +------- DataChannel ------> HID Events
//! ```

pub mod config;
pub mod h265_payloader;
pub mod peer;
pub mod rtp;
pub mod session;
pub mod signaling;
pub mod track;
pub mod universal_session;
pub mod video_track;
pub mod webrtc_streamer;

pub use config::WebRtcConfig;
pub use peer::PeerConnection;
pub use rtp::{H264VideoTrack, H264VideoTrackConfig, OpusAudioTrack};
pub use session::WebRtcSessionManager;
pub use signaling::{ConnectionState, IceCandidate, SdpAnswer, SdpOffer, SignalingMessage};
pub use universal_session::{UniversalSession, UniversalSessionConfig, UniversalSessionInfo};
pub use video_track::{
    UniversalVideoTrack, UniversalVideoTrackConfig, VideoCodec, VideoTrackStats,
};
pub use webrtc_streamer::{SessionInfo, WebRtcStreamer, WebRtcStreamerConfig, WebRtcStreamerStats};
