use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DiskMode {
    #[default]
    Single,
    Multi,
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

#[derive(Debug, Clone)]
pub struct MsdState {
    pub available: bool,
    pub disk_mode: DiskMode,
    pub mounted_media: Vec<MountedMedia>,
    pub drive_info: Option<DriveInfo>,
    pub usb_reenumerating: bool,
}

impl Default for MsdState {
    fn default() -> Self {
        Self {
            available: false,
            disk_mode: DiskMode::Single,
            mounted_media: Vec::new(),
            drive_info: None,
            usb_reenumerating: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MsdStateResponse {
    pub available: bool,
    pub disk_mode: DiskMode,
    pub slot_capacity: u8,
    pub mounted_count: u8,
    pub mounted_media: Vec<MountedMedia>,
    pub drive_info: Option<DriveInfo>,
    pub usb_reenumerating: bool,
}

impl From<&MsdState> for MsdStateResponse {
    fn from(state: &MsdState) -> Self {
        Self {
            available: state.available,
            disk_mode: state.disk_mode,
            slot_capacity: state.disk_mode.capacity(),
            mounted_count: state.mounted_media.len() as u8,
            mounted_media: state.mounted_media.clone(),
            drive_info: state.drive_info.clone(),
            usb_reenumerating: state.usb_reenumerating,
        }
    }
}

pub const SINGLE_DISK_MSD_LUNS: u8 = 1;
pub const MULTI_DISK_MSD_LUNS: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MountedMediaKind {
    Drive,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountedMedia {
    pub id: String,
    pub kind: MountedMediaKind,
    pub name: String,
    pub cdrom: bool,
    pub read_only: bool,
    pub size: u64,
    #[serde(skip)]
    pub lun: u8,
    #[serde(skip)]
    pub path: PathBuf,
}

impl MountedMedia {
    pub fn image(lun: u8, image: &ImageInfo, cdrom: bool, read_only: bool) -> Self {
        Self {
            id: image.id.clone(),
            lun,
            kind: MountedMediaKind::Image,
            name: image.name.clone(),
            cdrom,
            read_only: cdrom || read_only,
            size: image.size,
            path: image.path.clone(),
        }
    }

    pub fn drive(lun: u8, info: &DriveInfo) -> Self {
        Self {
            id: "drive".to_string(),
            lun,
            kind: MountedMediaKind::Drive,
            name: "Virtual USB".to_string(),
            cdrom: false,
            read_only: false,
            size: info.size,
            path: info.path.clone(),
        }
    }
}

impl DiskMode {
    pub fn capacity(self) -> u8 {
        match self {
            DiskMode::Single => SINGLE_DISK_MSD_LUNS,
            DiskMode::Multi => MULTI_DISK_MSD_LUNS,
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
pub struct DiskModeRequest {
    pub disk_mode: DiskMode,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageMountRequest {
    #[serde(default)]
    pub cdrom: bool,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriveInitRequest {
    #[serde(default = "default_drive_size")]
    pub size_mb: u32,
}

fn default_drive_size() -> u32 {
    64
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

    #[test]
    fn default_state_serializes_single_disk_mode() {
        assert_eq!(DiskMode::default(), DiskMode::Single);

        let state = MsdState::default();
        assert_eq!(state.disk_mode, DiskMode::Single);

        let json = serde_json::to_value(MsdStateResponse::from(&state)).unwrap();
        assert_eq!(json["disk_mode"], "single");
        assert_eq!(json["slot_capacity"], 1);
        assert!(json.get("mode").is_none());
        assert!(json.get("current_image").is_none());
        assert!(json.get("slots").is_none());
    }
}
