//! Low-latency WebRTC streaming: shared encoder → [`video_track::UniversalVideoTrack`] → peer;
//! HID over DataChannel.

pub mod config;
pub mod h265_payloader;
pub(crate) mod mdns;
pub mod rtp;
pub mod signaling;
pub mod universal_session;
pub mod video_track;
pub mod webrtc_streamer;

pub use config::WebRtcConfig;
pub use rtp::OpusAudioTrack;
pub use signaling::{ConnectionState, IceCandidate, SdpAnswer, SdpOffer, SignalingMessage};
pub use universal_session::{UniversalSession, UniversalSessionConfig, UniversalSessionInfo};
pub use video_track::{UniversalVideoTrack, UniversalVideoTrackConfig, VideoCodec};
pub use webrtc_streamer::{SessionInfo, WebRtcStreamer, WebRtcStreamerConfig, WebRtcStreamerStats};
