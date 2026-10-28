//! MJPEG multipart streaming.

pub mod mjpeg;
pub(crate) mod mjpeg_http;

pub use mjpeg::{ClientGuard, MjpegStreamHandler};
