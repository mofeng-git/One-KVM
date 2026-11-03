//! Native HDR capture: packed 10-bit PQ/BT.2020 from the HDMI receiver is
//! tone-mapped to SDR NV12 on the GPU and handed to the RKMPP encoder as a
//! dma-buf. No CPU pixel copies, and no per-frame format queries.
//!
//! Scope: 3840x2160 `NV20`/`NV15`, SMPTE ST 2084 over BT.2020. Anything else
//! is refused rather than converted with the wrong curve.

pub mod colorimetry;
pub mod layout;
pub mod pacing;
pub mod stage;

#[cfg(all(target_os = "linux", feature = "rkmpp"))]
mod encoder;
#[cfg(all(target_os = "linux", feature = "rkmpp"))]
mod vulkan;

#[cfg(all(target_os = "linux", feature = "rkmpp"))]
pub use encoder::{HdrConfig, HdrEncoder};
pub use stage::Packet;
