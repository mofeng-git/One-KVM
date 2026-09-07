use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use ventoy_img::{FileInfo as VentoyFileInfo, VentoyError, VentoyImage};

use super::types::{DriveFile, DriveFileAccess, DriveInfo};
use crate::error::{AppError, MsdErrorCode, Result};

const STREAM_CHUNK_SIZE: usize = 64 * 1024;

pub const MIN_DRIVE_SIZE_MB: u32 = 64;

const DEFAULT_LABEL: &str = "ONE-KVM";

pub struct VentoyDrive {
    path: PathBuf,
    lock: Arc<RwLock<()>>,
}

impl VentoyDrive {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Arc::new(RwLock::new(())),
        }
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Read and validate only the backing file metadata, without parsing its
    /// partition table or filesystem.
    pub fn raw_info(&self, file_access: DriveFileAccess) -> Result<DriveInfo> {
        raw_drive_info(&self.path, file_access)
    }

    pub async fn init(&self, size_mb: u32) -> Result<DriveInfo> {
        if size_mb < MIN_DRIVE_SIZE_MB {
            return Err(MsdErrorCode::MsdDriveSizeInvalid.into());
        }
        let size_str = format!("{}M", size_mb);
        let path = self.path.clone();
        let _lock = self.lock.write().await;

        info!("Creating {} MB Ventoy drive at {}", size_mb, path.display());

        let info = tokio::task::spawn_blocking(move || {
            VentoyImage::create(&path, &size_str, DEFAULT_LABEL).map_err(drive_init_error)?;

            let metadata = std::fs::metadata(&path)
                .map_err(|error| drive_io_error("read initialized drive metadata", error))?;

            Ok::<DriveInfo, AppError>(DriveInfo {
                size: metadata.len(),
                used: Some(0),
                free: Some(metadata.len()),
                initialized: true,
                file_access: DriveFileAccess::Available,
                path,
            })
        })
        .await
        .map_err(|error| task_error("initialize virtual drive", error))??;

        info!("Ventoy drive created successfully");
        Ok(info)
    }

    pub async fn info(&self) -> Result<DriveInfo> {
        let path = self.path.clone();
        let _lock = self.lock.read().await;

        tokio::task::spawn_blocking(move || {
            let raw = raw_drive_info(&path, DriveFileAccess::Unsupported)?;

            let image = match VentoyImage::open(&path) {
                Ok(image) => image,
                Err(error) if is_unsupported_filesystem_error(&error) => return Ok(raw),
                Err(error) => return Err(ventoy_to_app_error(error)),
            };

            let files = match image.list_files_recursive() {
                Ok(files) => files,
                Err(error) if is_unsupported_filesystem_error(&error) => return Ok(raw),
                Err(error) => return Err(ventoy_to_app_error(error)),
            };

            let used: u64 = files
                .iter()
                .filter(|f| !f.is_directory)
                .map(|f| f.size)
                .sum();

            let size = raw.size;
            let free = size.saturating_sub(used);

            Ok(DriveInfo {
                size,
                used: Some(used),
                free: Some(free),
                initialized: true,
                file_access: DriveFileAccess::Available,
                path,
            })
        })
        .await
        .map_err(|error| task_error("read virtual drive info", error))?
    }

    pub async fn list_files(&self, dir_path: &str) -> Result<Vec<DriveFile>> {
        if !self.exists() {
            return Err(MsdErrorCode::MsdDriveNotInitialized.into());
        }

        let path = self.path.clone();
        let dir_path = dir_path.to_string();
        let _lock = self.lock.read().await;

        tokio::task::spawn_blocking(move || {
            let image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            let files = if dir_path.is_empty() || dir_path == "/" {
                image.list_files()
            } else {
                image.list_files_at(&dir_path)
            }
            .map_err(ventoy_to_app_error)?;

            Ok(files
                .into_iter()
                .map(|f| ventoy_file_to_drive_file(f, &dir_path))
                .collect())
        })
        .await
        .map_err(|error| task_error("list virtual drive files", error))?
    }

    pub async fn write_file_from_multipart_field(
        &self,
        file_path: &str,
        mut field: axum::extract::multipart::Field<'_>,
    ) -> Result<u64> {
        if !self.exists() {
            return Err(MsdErrorCode::MsdDriveNotInitialized.into());
        }

        let temp_dir = self.path.parent().unwrap_or(Path::new("/tmp"));
        let temp_name = format!(".upload_ventoy_{}", uuid::Uuid::new_v4());
        let temp_path = temp_dir.join(&temp_name);

        let mut temp_file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|error| drive_io_error("create virtual drive upload", error))?;

        let mut bytes_written: u64 = 0;

        while let Some(chunk) = field.chunk().await.map_err(|error| {
            warn!(%error, "Failed to read virtual drive upload chunk");
            AppError::from(MsdErrorCode::MsdOperationFailed)
        })? {
            bytes_written += chunk.len() as u64;
            tokio::io::AsyncWriteExt::write_all(&mut temp_file, &chunk)
                .await
                .map_err(|error| drive_io_error("write virtual drive upload", error))?;
        }

        tokio::io::AsyncWriteExt::flush(&mut temp_file)
            .await
            .map_err(|error| drive_io_error("flush virtual drive upload", error))?;
        drop(temp_file);

        let path = self.path.clone();
        let file_path = file_path.to_string();
        let temp_path_clone = temp_path.clone();
        let _lock = self.lock.write().await;

        let result = tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            image
                .add_file_to_path(&temp_path_clone, &file_path, true, true)
                .map_err(ventoy_to_app_error)?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|error| task_error("write virtual drive file", error))?;

        let _ = tokio::fs::remove_file(&temp_path).await;

        result?;
        Ok(bytes_written)
    }

    #[cfg(test)]
    pub async fn read_file(&self, file_path: &str) -> Result<Vec<u8>> {
        if !self.exists() {
            return Err(MsdErrorCode::MsdDriveNotInitialized.into());
        }

        let path = self.path.clone();
        let file_path = file_path.to_string();
        let _lock = self.lock.read().await;

        tokio::task::spawn_blocking(move || {
            let image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            image.read_file(&file_path).map_err(ventoy_to_app_error)
        })
        .await
        .map_err(|error| task_error("read virtual drive file", error))?
    }

    pub async fn get_file_info(&self, file_path: &str) -> Result<Option<DriveFile>> {
        if !self.exists() {
            return Err(MsdErrorCode::MsdDriveNotInitialized.into());
        }

        let path = self.path.clone();
        let file_path_owned = file_path.to_string();
        let _lock = self.lock.read().await;

        let info = tokio::task::spawn_blocking(move || {
            let image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;
            image
                .get_file_info(&file_path_owned)
                .map_err(ventoy_to_app_error)
        })
        .await
        .map_err(|error| task_error("read virtual drive file information", error))??;

        Ok(info.map(|f| DriveFile {
            name: f.name,
            path: f.path,
            size: f.size,
            is_dir: f.is_directory,
            modified: None,
        }))
    }

    pub async fn read_file_stream(
        &self,
        file_path: &str,
    ) -> Result<(
        u64,
        tokio::sync::mpsc::Receiver<std::result::Result<bytes::Bytes, std::io::Error>>,
    )> {
        if !self.exists() {
            return Err(MsdErrorCode::MsdDriveNotInitialized.into());
        }

        let file_info = self
            .get_file_info(file_path)
            .await?
            .ok_or_else(|| AppError::from(MsdErrorCode::MsdResourceNotFound))?;

        if file_info.is_dir {
            return Err(MsdErrorCode::MsdInvalidRequest.into());
        }

        let file_size = file_info.size;
        let path = self.path.clone();
        let file_path_owned = file_path.to_string();
        let lock = self.lock.clone();

        let (tx, rx) =
            tokio::sync::mpsc::channel::<std::result::Result<bytes::Bytes, std::io::Error>>(8);

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            let _lock = rt.block_on(lock.read());

            let image = match VentoyImage::open(&path) {
                Ok(img) => img,
                Err(e) => {
                    let _ = rt.block_on(tx.send(Err(std::io::Error::other(e.to_string()))));
                    return;
                }
            };

            let mut chunk_writer = ChannelWriter::new(tx.clone(), rt.clone());

            if let Err(e) = image.read_file_to_writer(&file_path_owned, &mut chunk_writer) {
                let _ = rt.block_on(tx.send(Err(std::io::Error::other(e.to_string()))));
            }
        });

        Ok((file_size, rx))
    }

    pub async fn mkdir(&self, dir_path: &str) -> Result<()> {
        if !self.exists() {
            return Err(MsdErrorCode::MsdDriveNotInitialized.into());
        }

        let path = self.path.clone();
        let dir_path = dir_path.to_string();
        let _lock = self.lock.write().await;

        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            image
                .create_directory(&dir_path, true)
                .map_err(ventoy_to_app_error)
        })
        .await
        .map_err(|error| task_error("create virtual drive directory", error))?
    }

    pub async fn delete(&self, path_to_delete: &str) -> Result<()> {
        if !self.exists() {
            return Err(MsdErrorCode::MsdDriveNotInitialized.into());
        }

        let path = self.path.clone();
        let path_to_delete = path_to_delete.to_string();
        let _lock = self.lock.write().await;

        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            image
                .remove_recursive(&path_to_delete)
                .map_err(ventoy_to_app_error)
        })
        .await
        .map_err(|error| task_error("delete virtual drive resource", error))?
    }
}

fn raw_drive_info(path: &Path, file_access: DriveFileAccess) -> Result<DriveInfo> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::from(MsdErrorCode::MsdDriveNotInitialized)
        } else {
            drive_io_error("read drive metadata", error)
        }
    })?;

    if !metadata.is_file() || metadata.len() == 0 {
        return Err(MsdErrorCode::MsdDriveSizeInvalid.into());
    }

    Ok(DriveInfo::from_raw(
        path.to_path_buf(),
        metadata.len(),
        file_access,
    ))
}

fn is_unsupported_filesystem_error(error: &VentoyError) -> bool {
    matches!(
        error,
        VentoyError::FilesystemError(_)
            | VentoyError::ImageError(_)
            | VentoyError::PartitionError(_)
    )
}

fn ventoy_to_app_error(err: VentoyError) -> AppError {
    warn!(%err, "Virtual drive filesystem operation failed");
    match err {
        VentoyError::Io(error) => drive_io_error("access virtual drive", error),
        VentoyError::InvalidSize(_) | VentoyError::SizeParseError(_) => {
            MsdErrorCode::MsdDriveSizeInvalid.into()
        }
        VentoyError::FilesystemError(_)
        | VentoyError::ImageError(_)
        | VentoyError::PartitionError(_) => MsdErrorCode::MsdDriveFilesystemUnsupported.into(),
        VentoyError::FileNotFound(_) | VentoyError::ResourceNotFound(_) => {
            MsdErrorCode::MsdResourceNotFound.into()
        }
    }
}

fn drive_init_error(err: VentoyError) -> AppError {
    let VentoyError::Io(error) = err else {
        return ventoy_to_app_error(err);
    };

    #[cfg(unix)]
    match error.raw_os_error() {
        Some(libc::EFBIG) => MsdErrorCode::MsdDriveSizeInvalid.into(),
        Some(libc::ENOSPC) => MsdErrorCode::MsdStorageFull.into(),
        Some(libc::EROFS) => MsdErrorCode::MsdStorageReadOnly.into(),
        Some(libc::EACCES | libc::EPERM) => MsdErrorCode::MsdStoragePermissionDenied.into(),
        _ => drive_io_error("initialize virtual drive", error),
    }

    #[cfg(not(unix))]
    drive_io_error("initialize virtual drive", error)
}

fn drive_io_error(operation: &'static str, error: std::io::Error) -> AppError {
    warn!(operation, %error, "Virtual drive storage operation failed");
    #[cfg(unix)]
    let code = match error.raw_os_error() {
        Some(libc::EFBIG) => MsdErrorCode::MsdImageTooLarge,
        Some(libc::ENOSPC) => MsdErrorCode::MsdStorageFull,
        Some(libc::EROFS) => MsdErrorCode::MsdStorageReadOnly,
        Some(libc::EACCES | libc::EPERM) => MsdErrorCode::MsdStoragePermissionDenied,
        _ => MsdErrorCode::MsdOperationFailed,
    };
    #[cfg(not(unix))]
    let code = MsdErrorCode::MsdOperationFailed;
    code.into()
}

fn task_error(operation: &'static str, error: tokio::task::JoinError) -> AppError {
    warn!(operation, %error, "Virtual drive task failed");
    MsdErrorCode::MsdOperationFailed.into()
}

fn ventoy_file_to_drive_file(info: VentoyFileInfo, parent_path: &str) -> DriveFile {
    let full_path = if parent_path.is_empty() || parent_path == "/" {
        format!("/{}", info.name)
    } else {
        format!("{}/{}", parent_path.trim_end_matches('/'), info.name)
    };

    DriveFile {
        name: info.name,
        path: full_path,
        size: info.size,
        is_dir: info.is_directory,
        modified: None,
    }
}

struct ChannelWriter {
    tx: tokio::sync::mpsc::Sender<std::result::Result<bytes::Bytes, std::io::Error>>,
    rt: tokio::runtime::Handle,
    buffer: Vec<u8>,
}

impl ChannelWriter {
    fn new(
        tx: tokio::sync::mpsc::Sender<std::result::Result<bytes::Bytes, std::io::Error>>,
        rt: tokio::runtime::Handle,
    ) -> Self {
        Self {
            tx,
            rt,
            buffer: Vec::with_capacity(STREAM_CHUNK_SIZE),
        }
    }

    fn flush_buffer(&mut self) -> std::io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let chunk = bytes::Bytes::copy_from_slice(&self.buffer);
        self.buffer.clear();

        self.rt
            .block_on(self.tx.send(Ok(chunk)))
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Channel closed"))
    }
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut written = 0;

        while written < buf.len() {
            let space = STREAM_CHUNK_SIZE - self.buffer.len();
            let to_copy = std::cmp::min(space, buf.len() - written);

            self.buffer
                .extend_from_slice(&buf[written..written + to_copy]);
            written += to_copy;

            if self.buffer.len() >= STREAM_CHUNK_SIZE {
                self.flush_buffer()?;
            }
        }

        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flush_buffer()
    }
}

impl Drop for ChannelWriter {
    fn drop(&mut self) {
        let _ = self.flush_buffer();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use std::process::Command;
    use std::sync::OnceLock;
    use tempfile::TempDir;

    static RESOURCE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ventoy-img-rs/resources");

    #[test]
    fn classifies_drive_creation_io_errors() {
        for (errno, expected) in [
            (libc::EFBIG, MsdErrorCode::MsdDriveSizeInvalid),
            (libc::ENOSPC, MsdErrorCode::MsdStorageFull),
            (libc::EROFS, MsdErrorCode::MsdStorageReadOnly),
            (libc::EACCES, MsdErrorCode::MsdStoragePermissionDenied),
            (libc::EPERM, MsdErrorCode::MsdStoragePermissionDenied),
        ] {
            let error = drive_init_error(VentoyError::Io(std::io::Error::from_raw_os_error(errno)));
            assert!(matches!(error, AppError::Msd(error) if error.code() == expected));
        }
    }

    #[test]
    fn classifies_ventoy_filesystem_and_resource_errors() {
        for error in [
            VentoyError::FilesystemError("details".into()),
            VentoyError::ImageError("details".into()),
            VentoyError::PartitionError("details".into()),
        ] {
            assert!(matches!(
                ventoy_to_app_error(error),
                AppError::Msd(error) if error.code() == MsdErrorCode::MsdDriveFilesystemUnsupported
            ));
        }
        assert!(matches!(
            ventoy_to_app_error(VentoyError::FileNotFound("details".into())),
            AppError::Msd(error) if error.code() == MsdErrorCode::MsdResourceNotFound
        ));
    }

    fn init_ventoy_resources() -> bool {
        static INIT: OnceLock<bool> = OnceLock::new();
        *INIT.get_or_init(|| {
            let resource_path = std::path::Path::new(RESOURCE_DIR);

            let core_xz = resource_path.join("core.img.xz");
            let core_img = resource_path.join("core.img");
            if core_xz.exists() && !core_img.exists() {
                if let Err(e) = decompress_xz(&core_xz, &core_img) {
                    eprintln!("Failed to decompress core.img.xz: {}", e);
                    return false;
                }
            }

            let disk_xz = resource_path.join("ventoy.disk.img.xz");
            let disk_img = resource_path.join("ventoy.disk.img");
            if disk_xz.exists() && !disk_img.exists() {
                if let Err(e) = decompress_xz(&disk_xz, &disk_img) {
                    eprintln!("Failed to decompress ventoy.disk.img.xz: {}", e);
                    return false;
                }
            }

            if let Err(e) = ventoy_img::resources::init_resources(resource_path) {
                eprintln!("Failed to init ventoy resources: {}", e);
                return false;
            }

            true
        })
    }

    fn decompress_xz(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        let output = Command::new("xz")
            .args(["-d", "-k", "-c", src.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            return Err(std::io::Error::other(format!(
                "xz decompress failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        std::fs::write(dst, &output.stdout)?;
        Ok(())
    }

    fn ensure_resources() -> bool {
        if !init_ventoy_resources() {
            eprintln!("Skipping test: ventoy resources not available");
            false
        } else {
            true
        }
    }

    #[tokio::test]
    async fn test_drive_init() {
        if !ensure_resources() {
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let drive_path = temp_dir.path().join("test_ventoy.img");
        let drive = VentoyDrive::new(drive_path);

        let info = drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();
        assert!(info.initialized);
        assert_eq!(info.file_access, DriveFileAccess::Available);
        assert_eq!(info.used, Some(0));
        assert!(info.free.is_some());
        assert!(drive.exists());
    }

    #[tokio::test]
    async fn raw_bytes_are_reported_as_unsupported_with_capacity() {
        let temp_dir = TempDir::new().unwrap();
        let drive_path = temp_dir.path().join("custom.img");
        std::fs::write(&drive_path, vec![0x5a; 1024 * 1024]).unwrap();
        let drive = VentoyDrive::new(drive_path);

        let info = drive.info().await.unwrap();
        assert_eq!(info.size, 1024 * 1024);
        assert_eq!(info.used, None);
        assert_eq!(info.free, None);
        assert_eq!(info.file_access, DriveFileAccess::Unsupported);

        assert!(matches!(
            drive.list_files("/").await.unwrap_err(),
            AppError::Msd(error)
                if error.code() == MsdErrorCode::MsdDriveFilesystemUnsupported
        ));
    }

    #[test]
    fn raw_metadata_rejects_missing_empty_and_non_file_paths() {
        let temp_dir = TempDir::new().unwrap();
        let missing = VentoyDrive::new(temp_dir.path().join("missing.img"));
        assert!(matches!(
            missing.raw_info(DriveFileAccess::Unknown).unwrap_err(),
            AppError::Msd(error) if error.code() == MsdErrorCode::MsdDriveNotInitialized
        ));

        let empty_path = temp_dir.path().join("empty.img");
        std::fs::write(&empty_path, []).unwrap();
        let empty = VentoyDrive::new(empty_path);
        assert!(matches!(
            empty.raw_info(DriveFileAccess::Unknown).unwrap_err(),
            AppError::Msd(error) if error.code() == MsdErrorCode::MsdDriveSizeInvalid
        ));

        let directory = VentoyDrive::new(temp_dir.path().to_path_buf());
        assert!(matches!(
            directory.raw_info(DriveFileAccess::Unknown).unwrap_err(),
            AppError::Msd(error) if error.code() == MsdErrorCode::MsdDriveSizeInvalid
        ));
    }

    #[tokio::test]
    async fn supported_drive_info_has_space_values() {
        if !ensure_resources() {
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let drive = VentoyDrive::new(temp_dir.path().join("supported.img"));
        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();

        let info = drive.info().await.unwrap();
        assert_eq!(info.file_access, DriveFileAccess::Available);
        assert!(info.used.is_some());
        assert!(info.free.is_some());
    }

    #[tokio::test]
    async fn test_drive_mkdir() {
        if !ensure_resources() {
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let drive_path = temp_dir.path().join("test_ventoy.img");
        let drive = VentoyDrive::new(drive_path);

        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();
        drive.mkdir("/isos").await.unwrap();

        let files = drive.list_files("/").await.unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].is_dir);
        assert_eq!(files[0].name, "isos");
    }

    #[tokio::test]
    async fn test_drive_file_write_and_read() {
        if !ensure_resources() {
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let drive_path = temp_dir.path().join("test_ventoy.img");
        let drive = VentoyDrive::new(drive_path.clone());

        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();

        let test_content = b"Hello, Ventoy!";
        let test_file_path = temp_dir.path().join("test.txt");
        std::fs::write(&test_file_path, test_content).unwrap();

        let path = drive.path().clone();
        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).unwrap();
            image.add_file(&test_file_path).unwrap();
        })
        .await
        .unwrap();

        let read_data = drive.read_file("/test.txt").await.unwrap();
        assert_eq!(read_data, test_content);
    }

    #[tokio::test]
    async fn test_drive_get_file_info() {
        if !ensure_resources() {
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let drive_path = temp_dir.path().join("test_ventoy.img");
        let drive = VentoyDrive::new(drive_path.clone());

        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();

        drive.mkdir("/mydir").await.unwrap();

        let test_content = b"Test file content for info check";
        let test_file_path = temp_dir.path().join("info_test.txt");
        std::fs::write(&test_file_path, test_content).unwrap();

        let path = drive.path().clone();
        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).unwrap();
            image.add_file(&test_file_path).unwrap();
        })
        .await
        .unwrap();

        let file_info = drive.get_file_info("/info_test.txt").await.unwrap();
        assert!(file_info.is_some());
        let file_info = file_info.unwrap();
        assert_eq!(file_info.name, "info_test.txt");
        assert_eq!(file_info.size, test_content.len() as u64);
        assert!(!file_info.is_dir);

        let dir_info = drive.get_file_info("/mydir").await.unwrap();
        assert!(dir_info.is_some());
        let dir_info = dir_info.unwrap();
        assert_eq!(dir_info.name, "mydir");
        assert!(dir_info.is_dir);

        let not_found = drive.get_file_info("/nonexistent.txt").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_drive_stream_read() {
        if !ensure_resources() {
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let drive_path = temp_dir.path().join("test_ventoy.img");
        let drive = VentoyDrive::new(drive_path.clone());

        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();

        let test_size = 200 * 1024;
        let test_content: Vec<u8> = (0..test_size).map(|i| (i % 256) as u8).collect();
        let test_file_path = temp_dir.path().join("large_file.bin");
        std::fs::write(&test_file_path, &test_content).unwrap();

        let path = drive.path().clone();
        let file_path_clone = test_file_path.clone();
        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).unwrap();
            image.add_file(&file_path_clone).unwrap();
        })
        .await
        .unwrap();

        let (file_size, mut rx) = drive.read_file_stream("/large_file.bin").await.unwrap();
        assert_eq!(file_size, test_size as u64);

        let mut received_data = Vec::new();
        while let Some(chunk_result) = rx.recv().await {
            let chunk = chunk_result.expect("Chunk should not be an error");
            received_data.extend_from_slice(&chunk);
        }

        assert_eq!(received_data.len(), test_content.len());
        assert_eq!(received_data, test_content);
    }

    #[tokio::test]
    async fn test_drive_stream_read_small_file() {
        if !ensure_resources() {
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let drive_path = temp_dir.path().join("test_ventoy.img");
        let drive = VentoyDrive::new(drive_path.clone());

        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();

        let test_content = b"Small file for streaming test";
        let test_file_path = temp_dir.path().join("small.txt");
        std::fs::write(&test_file_path, test_content).unwrap();

        let path = drive.path().clone();
        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).unwrap();
            image.add_file(&test_file_path).unwrap();
        })
        .await
        .unwrap();

        let (file_size, mut rx) = drive.read_file_stream("/small.txt").await.unwrap();
        assert_eq!(file_size, test_content.len() as u64);

        let mut received_data = Vec::new();
        while let Some(chunk_result) = rx.recv().await {
            let chunk = chunk_result.expect("Chunk should not be an error");
            received_data.extend_from_slice(&chunk);
        }

        assert_eq!(received_data.as_slice(), test_content);
    }
}
