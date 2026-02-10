//! MSD data types and structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// MSD operating mode
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum MsdMode {
    /// No storage connected
    #[default]
    None,
    /// Image file mounted (ISO/IMG)
    Image,
    /// Virtual drive (FAT32) connected
    Drive,
}


/// Image file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    /// Unique image ID
    pub id: String,
    /// Display name
    pub name: String,
    /// File path on disk
    #[serde(skip_serializing)]
    pub path: PathBuf,
    /// File size in bytes
    pub size: u64,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl ImageInfo {
    /// Create new image info
    pub fn new(id: String, name: String, path: PathBuf, size: u64) -> Self {
        Self {
            id,
            name,
            path,
            size,
            created_at: Utc::now(),
        }
    }

    /// Format size for display
    pub fn size_display(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if self.size >= GB {
            format!("{:.2} GB", self.size as f64 / GB as f64)
        } else if self.size >= MB {
            format!("{:.2} MB", self.size as f64 / MB as f64)
        } else if self.size >= KB {
            format!("{:.2} KB", self.size as f64 / KB as f64)
        } else {
            format!("{} B", self.size)
        }
    }
}

/// MSD state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsdState {
    /// Whether MSD feature is available
    pub available: bool,
    /// Current mode
    pub mode: MsdMode,
    /// Whether storage is connected to target
    pub connected: bool,
    /// Currently mounted image (if mode is Image)
    pub current_image: Option<ImageInfo>,
    /// Virtual drive info (if mode is Drive)
    pub drive_info: Option<DriveInfo>,
}

impl Default for MsdState {
    fn default() -> Self {
        Self {
            available: false,
            mode: MsdMode::None,
            connected: false,
            current_image: None,
            drive_info: None,
        }
    }
}

/// Virtual drive information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    /// Drive size in bytes
    pub size: u64,
    /// Used space in bytes
    pub used: u64,
    /// Free space in bytes
    pub free: u64,
    /// Whether drive is initialized
    pub initialized: bool,
    /// Drive file path
    #[serde(skip_serializing)]
    pub path: PathBuf,
}

impl DriveInfo {
    /// Create new drive info
    pub fn new(path: PathBuf, size: u64) -> Self {
        Self {
            size,
            used: 0,
            free: size,
            initialized: false,
            path,
        }
    }
}

/// File entry in virtual drive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveFile {
    /// File name
    pub name: String,
    /// Relative path from drive root
    pub path: String,
    /// File size in bytes (0 for directories)
    pub size: u64,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Last modified timestamp
    pub modified: Option<DateTime<Utc>>,
}

/// MSD connect request
#[derive(Debug, Clone, Deserialize)]
pub struct MsdConnectRequest {
    /// Connection mode: "image" or "drive"
    pub mode: MsdMode,
    /// Image ID to mount (required for image mode)
    pub image_id: Option<String>,
    /// Mount as CD-ROM (optional, defaults based on image type)
    #[serde(default)]
    pub cdrom: Option<bool>,
    /// Mount as read-only
    #[serde(default)]
    pub read_only: Option<bool>,
}

/// Virtual drive init request
#[derive(Debug, Clone, Deserialize)]
pub struct DriveInitRequest {
    /// Drive size in megabytes (defaults to 16GB)
    #[serde(default = "default_drive_size")]
    pub size_mb: u32,
    /// Optional custom path for Ventoy installation
    pub ventoy_path: Option<String>,
}

fn default_drive_size() -> u32 {
    16 * 1024 // 16GB
}

/// Image download request
#[derive(Debug, Clone, Deserialize)]
pub struct ImageDownloadRequest {
    /// URL to download from
    pub url: String,
    /// Optional custom filename
    pub filename: Option<String>,
}

/// Download status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    /// Download has started
    Started,
    /// Download is in progress
    InProgress,
    /// Download completed successfully
    Completed,
    /// Download failed
    Failed,
}

/// Download progress information
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    /// Unique download ID
    pub download_id: String,
    /// Source URL
    pub url: String,
    /// Target filename
    pub filename: String,
    /// Bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Total file size (None if unknown)
    pub total_bytes: Option<u64>,
    /// Progress percentage (0.0 - 100.0, None if total unknown)
    pub progress_pct: Option<f32>,
    /// Download status
    pub status: DownloadStatus,
    /// Error message if failed
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_display() {
        let info = ImageInfo::new(
            "test".into(),
            "test.iso".into(),
            PathBuf::from("/tmp/test.iso"),
            1024 * 1024 * 1024 * 2, // 2 GB
        );
        assert!(info.size_display().contains("GB"));
    }
}
