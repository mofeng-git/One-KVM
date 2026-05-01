use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MsdMode {
    #[default]
    None,
    Image,
    Drive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub path: PathBuf,
    pub size: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl ImageInfo {
    pub fn new(id: String, name: String, path: PathBuf, size: u64) -> Self {
        Self {
            id,
            name,
            path,
            size,
            created_at: OffsetDateTime::now_utc(),
        }
    }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsdState {
    pub available: bool,
    pub mode: MsdMode,
    pub connected: bool,
    pub current_image: Option<ImageInfo>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub size: u64,
    pub used: u64,
    pub free: u64,
    pub initialized: bool,
    #[serde(skip_serializing)]
    pub path: PathBuf,
}

impl DriveInfo {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub modified: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MsdConnectRequest {
    pub mode: MsdMode,
    pub image_id: Option<String>,
    #[serde(default)]
    pub cdrom: Option<bool>,
    #[serde(default)]
    pub read_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriveInitRequest {
    #[serde(default = "default_drive_size")]
    pub size_mb: u32,
}

fn default_drive_size() -> u32 {
    16 * 1024
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageDownloadRequest {
    pub url: String,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Started,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub download_id: String,
    pub url: String,
    pub filename: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub progress_pct: Option<f32>,
    pub status: DownloadStatus,
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
            1024 * 1024 * 1024 * 2,
        );
        assert!(info.size_display().contains("GB"));
    }
}
