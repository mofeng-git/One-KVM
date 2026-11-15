//! RTSP TCP server exposing H.264/H.265 video from [`VideoStreamManager`](crate::video::VideoStreamManager).

#[cfg(feature = "rtsp")]
mod auth;
#[cfg(feature = "rtsp")]
mod bitstream;
#[cfg(feature = "rtsp")]
mod codec;
#[cfg(feature = "rtsp")]
mod protocol;
#[cfg(feature = "rtsp")]
mod response;
#[cfg(feature = "rtsp")]
mod sdp;
#[cfg(feature = "rtsp")]
mod service;
#[cfg(feature = "rtsp")]
mod state;
#[cfg(feature = "rtsp")]
mod streaming;
mod types;

pub use types::RtspServiceStatus;
#[cfg(feature = "rtsp")]
pub use service::RtspService;
