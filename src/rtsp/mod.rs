//! RTSP TCP server exposing H.264/H.265 video from [`VideoStreamManager`](crate::video::VideoStreamManager).

mod auth;
mod bitstream;
mod codec;
mod protocol;
mod response;
mod sdp;
mod service;
mod state;
mod streaming;
mod types;

pub use service::{RtspService, RtspServiceStatus};
