//! MJPEG multipart streaming and WebSocket HID (for MJPEG mode).

pub mod mjpeg;
pub(crate) mod mjpeg_http;
#[cfg(feature = "desktop")]
pub mod ws_hid;

pub use mjpeg::{ClientGuard, MjpegStreamHandler};
#[cfg(feature = "desktop")]
pub use ws_hid::WsHidHandler;
