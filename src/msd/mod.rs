//! MSD (Mass Storage Device) module
//!
//! Provides virtual USB storage functionality with two modes:
//! - Image mounting: Mount ISO/IMG files for system installation
//! - Ventoy drive: Bootable exFAT drive for multiple ISO files
//!
//! Architecture:
//! ```text
//! Web API --> MSD Controller --> ConfigFS Mass Storage --> Target PC
//!                 |
//!          ┌──────┴──────┐
//!          │             │
//!    Image Manager  Ventoy Drive
//!    (ISO/IMG)      (Bootable exFAT)
//! ```

pub mod controller;
pub mod image;
pub mod monitor;
pub mod types;
pub mod ventoy_drive;

pub use controller::MsdController;
pub use image::ImageManager;
pub use monitor::{MsdHealthMonitor, MsdHealthStatus, MsdMonitorConfig};
pub use types::{
    DownloadProgress, DownloadStatus, DriveFile, DriveInfo, DriveInitRequest, ImageDownloadRequest,
    ImageInfo, MsdConnectRequest, MsdMode, MsdState,
};
pub use ventoy_drive::VentoyDrive;

// Re-export from otg module for backward compatibility
pub use crate::otg::{MsdFunction, MsdLunConfig};
