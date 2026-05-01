//! MJPEG multipart streaming and WebSocket HID (for MJPEG mode).

pub mod mjpeg;
pub mod ws_hid;

pub use mjpeg::{ClientGuard, MjpegStreamHandler};
pub use ws_hid::WsHidHandler;
