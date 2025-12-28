//! OTG USB Gadget unified management module
//!
//! This module provides unified management for USB Gadget functions:
//! - HID (Keyboard, Mouse)
//! - MSD (Mass Storage Device)
//!
//! Architecture:
//! ```text
//! OtgService (high-level coordination)
//!     └── OtgGadgetManager (gadget lifecycle)
//!             ├── EndpointAllocator (manages UDC endpoints)
//!             ├── HidFunction (keyboard, mouse_rel, mouse_abs)
//!             └── MsdFunction (mass storage)
//! ```
//!
//! The recommended way to use this module is through `OtgService`, which provides
//! a high-level interface for enabling/disabling HID and MSD functions independently.
//! Both `HidController` and `MsdController` should share the same `OtgService` instance.

pub mod configfs;
pub mod endpoint;
pub mod function;
pub mod hid;
pub mod manager;
pub mod msd;
pub mod report_desc;
pub mod service;

pub use endpoint::EndpointAllocator;
pub use function::{FunctionMeta, GadgetFunction};
pub use hid::{HidFunction, HidFunctionType};
pub use manager::{wait_for_hid_devices, OtgGadgetManager};
pub use msd::{MsdFunction, MsdLunConfig};
pub use report_desc::{KEYBOARD_WITH_LED, MOUSE_ABSOLUTE, MOUSE_RELATIVE};
pub use service::{HidDevicePaths, OtgService, OtgServiceState};
