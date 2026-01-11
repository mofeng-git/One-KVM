//! Ventoy IMG Generator
//!
//! A Rust library for creating and managing Ventoy bootable IMG files
//! without requiring root privileges or loop devices.
//!
//! # Features
//!
//! - Create Ventoy IMG files with MBR partition table
//! - Format data partition as exFAT
//! - Add, list, read, and remove files in the data partition
//! - Load boot resources from external files
//!
//! # Example
//!
//! ```no_run
//! use ventoy_img::{VentoyImage, resources};
//! use std::path::Path;
//!
//! // Initialize resources from data directory
//! resources::init_resources(Path::new("/var/lib/one-kvm/ventoy")).unwrap();
//!
//! // Create a new 8GB Ventoy image
//! let mut image = VentoyImage::create(
//!     Path::new("ventoy.img"),
//!     "8G",
//!     "Ventoy"
//! ).unwrap();
//!
//! // Add an ISO file
//! image.add_file(Path::new("/path/to/ubuntu.iso")).unwrap();
//!
//! // List files
//! for file in image.list_files().unwrap() {
//!     println!("{}: {} bytes", file.name, file.size);
//! }
//! ```

pub mod error;
pub mod exfat;
pub mod image;
pub mod partition;
pub mod resources;

pub use error::{Result, VentoyError};
pub use exfat::FileInfo;
pub use image::VentoyImage;
pub use partition::{parse_size, PartitionLayout};
pub use resources::{get_resource_dir, init_resources, is_initialized, required_files};
