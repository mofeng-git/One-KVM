//! Video streaming module
//!
//! Provides MJPEG streaming and WebSocket handlers for MJPEG mode.
//!
//! # Components
//!
//! - `MjpegStreamer` - High-level MJPEG streaming manager
//! - `MjpegStreamHandler` - HTTP multipart MJPEG video streaming
//! - `WsHidHandler` - WebSocket HID input handler

pub mod mjpeg;
pub mod mjpeg_streamer;
pub mod ws_hid;

pub use mjpeg::{ClientGuard, MjpegStreamHandler};
pub use mjpeg_streamer::{
    MjpegStreamer, MjpegStreamerConfig, MjpegStreamerState, MjpegStreamerStats,
};
pub use ws_hid::WsHidHandler;
