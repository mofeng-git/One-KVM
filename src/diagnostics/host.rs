//! Read-only host metadata and filesystem identity for the About page.
use super::{get_disk_space, DiskSpaceInfo};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct HostInfo {
    pub operating_system: String,
    pub architecture: &'static str,
    pub kernel: Option<String>,
    pub cpu_count: usize,
    pub storage: StorageInfo,
}

#[derive(Debug, Serialize)]
pub struct StorageInfo {
    pub system: Option<DiskSpaceInfo>,
    pub msd: Option<DiskSpaceInfo>,
    /// Unknown identity must never be interpreted as a shared filesystem.
    pub shared_partition: Option<bool>,
}

pub fn get_host_info(msd_path: &Path) -> HostInfo {
    let operating_system = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|text| os_release_name(&text))
        .unwrap_or_else(|| std::env::consts::OS.to_string());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    HostInfo {
        operating_system,
        architecture: std::env::consts::ARCH,
        kernel,
        cpu_count: std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        storage: storage_info(system_root().as_path(), msd_path),
    }
}

fn system_root() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(std::env::var_os("SystemDrive").unwrap_or_else(|| "C:".into())).join("\\")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/")
    }
}

fn os_release_name(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let value = line
            .strip_prefix("PRETTY_NAME=")?
            .trim()
            .trim_matches(['\'', '"']);
        (!value.is_empty()).then(|| value.to_string())
    })
}

// MSD directories may not have been created yet. Read their containing filesystem
// without creating directories or changing permissions.
fn existing_ancestor(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }
    let mut candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    loop {
        match std::fs::metadata(&candidate) {
            Ok(_) => return Some(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !candidate.pop() {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
}

fn storage_info(system: &Path, msd: &Path) -> StorageInfo {
    let system = existing_ancestor(system);
    let msd = existing_ancestor(msd);
    StorageInfo {
        system: system.as_deref().and_then(|path| get_disk_space(path).ok()),
        msd: msd.as_deref().and_then(|path| get_disk_space(path).ok()),
        shared_partition: system
            .as_deref()
            .zip(msd.as_deref())
            .and_then(|(a, b)| same_filesystem(a, b)),
    }
}

#[cfg(unix)]
fn same_filesystem(a: &Path, b: &Path) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    Some(std::fs::metadata(a).ok()?.dev() == std::fs::metadata(b).ok()?.dev())
}

#[cfg(not(unix))]
fn same_filesystem(_a: &Path, _b: &Path) -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_name_handles_quotes_and_missing_values() {
        assert_eq!(
            os_release_name("NAME=Linux\nPRETTY_NAME=\"OpenWrt 24.10\"\n"),
            Some("OpenWrt 24.10".into())
        );
        assert_eq!(os_release_name("PRETTY_NAME=\n"), None);
        assert_eq!(os_release_name("NAME=Linux\n"), None);
    }

    #[cfg(unix)]
    #[test]
    fn nested_msd_directory_shares_its_existing_parent_filesystem() {
        let parent = tempfile::tempdir().unwrap();
        let missing = parent.path().join("not-created/msd");
        let storage = storage_info(parent.path(), &missing);
        assert_eq!(storage.shared_partition, Some(true));
        assert!(storage.system.is_some() && storage.msd.is_some());
        assert!(!missing.exists());
    }

    #[test]
    fn missing_msd_is_not_a_duplicate_system_partition() {
        let storage = storage_info(&system_root(), Path::new(""));
        assert!(storage.msd.is_none());
        assert_eq!(storage.shared_partition, None);
    }
}
