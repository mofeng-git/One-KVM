//! Video streaming module
//!
//! Provides MJPEG streaming and WebSocket handlers for MJPEG mode.
//!
//! # Components
//!
//! - `MjpegStreamHandler` - HTTP multipart MJPEG video streaming
//! - `WsHidHandler` - WebSocket HID input handler

pub mod mjpeg;
pub mod ws_hid;

pub use mjpeg::{ClientGuard, MjpegStreamHandler};
pub use ws_hid::WsHidHandler;
