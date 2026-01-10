//! Ventoy Virtual Drive
//!
//! Replaces FAT32 VirtualDrive with a Ventoy bootable image.
//! Provides a bootable USB with exFAT data partition for ISO files.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use ventoy_img::{FileInfo as VentoyFileInfo, VentoyError, VentoyImage};

use super::types::{DriveFile, DriveInfo};
use crate::error::{AppError, Result};

/// Chunk size for streaming reads (64 KB)
const STREAM_CHUNK_SIZE: usize = 64 * 1024;

/// Minimum drive size (1 GB) - Ventoy requires space for boot partition
const MIN_DRIVE_SIZE_MB: u32 = 1024;

/// Maximum drive size (128 GB)
const MAX_DRIVE_SIZE_MB: u32 = 128 * 1024;

/// Default drive label
const DEFAULT_LABEL: &str = "ONE-KVM";

/// Ventoy Drive Manager
///
/// Thread-safe wrapper around VentoyImage providing async file operations.
/// Uses spawn_blocking for all ventoy-img-rs operations since they are synchronous.
/// Uses RwLock to allow concurrent read operations while serializing writes.
pub struct VentoyDrive {
    /// Drive image path
    path: PathBuf,
    /// RwLock for concurrent reads, exclusive writes
    /// (ventoy-img-rs operations are synchronous and not thread-safe)
    lock: Arc<RwLock<()>>,
}

impl VentoyDrive {
    /// Create new Ventoy drive manager
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Arc::new(RwLock::new(())),
        }
    }

    /// Check if drive image exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Get drive path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Initialize a new Ventoy drive image
    ///
    /// Creates a bootable Ventoy image with the specified size.
    /// The image includes boot partitions and an exFAT data partition.
    pub async fn init(&self, size_mb: u32) -> Result<DriveInfo> {
        let size_mb = size_mb.clamp(MIN_DRIVE_SIZE_MB, MAX_DRIVE_SIZE_MB);
        let size_str = format!("{}M", size_mb);
        let path = self.path.clone();
        let _lock = self.lock.write().await; // Write lock for initialization

        info!("Creating {} MB Ventoy drive at {}", size_mb, path.display());

        // Run Ventoy creation in blocking task
        let info = tokio::task::spawn_blocking(move || {
            VentoyImage::create(&path, &size_str, DEFAULT_LABEL)
                .map_err(ventoy_to_app_error)?;

            // Get file metadata for DriveInfo
            let metadata = std::fs::metadata(&path).map_err(|e| {
                AppError::Internal(format!("Failed to read drive metadata: {}", e))
            })?;

            Ok::<DriveInfo, AppError>(DriveInfo {
                size: metadata.len(),
                used: 0,
                free: metadata.len(), // Approximate - exFAT overhead not calculated
                initialized: true,
                path,
            })
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))??;

        info!("Ventoy drive created successfully");
        Ok(info)
    }

    /// Get drive information
    pub async fn info(&self) -> Result<DriveInfo> {
        if !self.exists() {
            return Err(AppError::Internal("Drive not initialized".to_string()));
        }

        let path = self.path.clone();
        let _lock = self.lock.read().await; // Read lock for info query

        tokio::task::spawn_blocking(move || {
            let metadata = std::fs::metadata(&path).map_err(|e| {
                AppError::Internal(format!("Failed to read drive metadata: {}", e))
            })?;

            // Open image to get file list and calculate used space
            let image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            let files = image
                .list_files_recursive()
                .map_err(ventoy_to_app_error)?;

            let used: u64 = files
                .iter()
                .filter(|f| !f.is_directory)
                .map(|f| f.size)
                .sum();

            // Note: This is approximate since we don't have exact exFAT overhead
            let size = metadata.len();
            let free = size.saturating_sub(used);

            Ok(DriveInfo {
                size,
                used,
                free,
                initialized: true,
                path,
            })
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
    }

    /// List files at a given path (or root if empty/"/")
    pub async fn list_files(&self, dir_path: &str) -> Result<Vec<DriveFile>> {
        if !self.exists() {
            return Err(AppError::Internal("Drive not initialized".to_string()));
        }

        let path = self.path.clone();
        let dir_path = dir_path.to_string();
        let _lock = self.lock.read().await; // Read lock for listing

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
        .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
    }

    /// Write a file to the drive from multipart upload (streaming)
    ///
    /// Streams the file directly into the Ventoy image's exFAT partition.
    pub async fn write_file_from_multipart_field(
        &self,
        file_path: &str,
        mut field: axum::extract::multipart::Field<'_>,
    ) -> Result<u64> {
        if !self.exists() {
            return Err(AppError::Internal("Drive not initialized".to_string()));
        }

        // First, stream to a temporary file (to get the size)
        let temp_dir = self.path.parent().unwrap_or(Path::new("/tmp"));
        let temp_name = format!(".upload_ventoy_{}", uuid::Uuid::new_v4());
        let temp_path = temp_dir.join(&temp_name);

        // Stream upload to temp file
        let mut temp_file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create temp file: {}", e)))?;

        let mut bytes_written: u64 = 0;

        while let Some(chunk) = field.chunk().await.map_err(|e| {
            AppError::Internal(format!("Failed to read upload chunk: {}", e))
        })? {
            bytes_written += chunk.len() as u64;
            tokio::io::AsyncWriteExt::write_all(&mut temp_file, &chunk)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to write chunk: {}", e)))?;
        }

        tokio::io::AsyncWriteExt::flush(&mut temp_file)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to flush temp file: {}", e)))?;
        drop(temp_file);

        // Now copy from temp file to Ventoy image
        let path = self.path.clone();
        let file_path = file_path.to_string();
        let temp_path_clone = temp_path.clone();
        let _lock = self.lock.write().await; // Write lock for file write

        let result = tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            // Use add_file_to_path which handles streaming internally
            image
                .add_file_to_path(
                    &temp_path_clone,
                    &file_path,
                    true, // create_parents
                    true, // overwrite
                )
                .map_err(ventoy_to_app_error)?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?;

        // Cleanup temp file
        let _ = tokio::fs::remove_file(&temp_path).await;

        result?;
        Ok(bytes_written)
    }

    /// Read a file from the drive (for download)
    pub async fn read_file(&self, file_path: &str) -> Result<Vec<u8>> {
        if !self.exists() {
            return Err(AppError::Internal("Drive not initialized".to_string()));
        }

        let path = self.path.clone();
        let file_path = file_path.to_string();
        let _lock = self.lock.read().await; // Read lock for file read

        tokio::task::spawn_blocking(move || {
            let image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            image
                .read_file(&file_path)
                .map_err(ventoy_to_app_error)
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
    }

    /// Get file information without reading content
    ///
    /// Returns file size, name, and other metadata.
    /// Returns None if the file doesn't exist.
    pub async fn get_file_info(&self, file_path: &str) -> Result<Option<DriveFile>> {
        if !self.exists() {
            return Err(AppError::Internal("Drive not initialized".to_string()));
        }

        let path = self.path.clone();
        let file_path_owned = file_path.to_string();
        let _lock = self.lock.read().await; // Read lock for file info

        let info = tokio::task::spawn_blocking(move || {
            let image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;
            image
                .get_file_info(&file_path_owned)
                .map_err(ventoy_to_app_error)
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))??;

        Ok(info.map(|f| DriveFile {
            name: f.name,
            path: f.path,
            size: f.size,
            is_dir: f.is_directory,
            modified: None,
        }))
    }

    /// Read a file from the drive as a stream (for large file downloads)
    ///
    /// Returns an async channel receiver that yields chunks of file data.
    /// This avoids loading the entire file into memory.
    pub async fn read_file_stream(
        &self,
        file_path: &str,
    ) -> Result<(
        u64,
        tokio::sync::mpsc::Receiver<std::result::Result<bytes::Bytes, std::io::Error>>,
    )> {
        if !self.exists() {
            return Err(AppError::Internal("Drive not initialized".to_string()));
        }

        // First, get the file size
        let file_info = self
            .get_file_info(file_path)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("File not found: {}", file_path)))?;

        if file_info.is_dir {
            return Err(AppError::BadRequest(format!(
                "'{}' is a directory",
                file_path
            )));
        }

        let file_size = file_info.size;
        let path = self.path.clone();
        let file_path_owned = file_path.to_string();
        let lock = self.lock.clone();

        // Create a channel for streaming data
        let (tx, rx) = tokio::sync::mpsc::channel::<std::result::Result<bytes::Bytes, std::io::Error>>(8);

        // Spawn blocking task to read and send chunks
        tokio::task::spawn_blocking(move || {
            // Hold read lock for the entire read operation
            let rt = tokio::runtime::Handle::current();
            let _lock = rt.block_on(lock.read()); // Read lock for streaming

            let image = match VentoyImage::open(&path) {
                Ok(img) => img,
                Err(e) => {
                    let _ = rt.block_on(tx.send(Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e.to_string(),
                    ))));
                    return;
                }
            };

            // Create a channel writer that sends chunks
            let mut chunk_writer = ChannelWriter::new(tx.clone(), rt.clone());

            // Stream the file through the writer
            if let Err(e) = image.read_file_to_writer(&file_path_owned, &mut chunk_writer) {
                let _ = rt.block_on(tx.send(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))));
            }
        });

        Ok((file_size, rx))
    }

    /// Create a directory
    pub async fn mkdir(&self, dir_path: &str) -> Result<()> {
        if !self.exists() {
            return Err(AppError::Internal("Drive not initialized".to_string()));
        }

        let path = self.path.clone();
        let dir_path = dir_path.to_string();
        let _lock = self.lock.write().await; // Write lock for mkdir

        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            image
                .create_directory(&dir_path, true)
                .map_err(ventoy_to_app_error)
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
    }

    /// Delete a file or directory
    pub async fn delete(&self, path_to_delete: &str) -> Result<()> {
        if !self.exists() {
            return Err(AppError::Internal("Drive not initialized".to_string()));
        }

        let path = self.path.clone();
        let path_to_delete = path_to_delete.to_string();
        let _lock = self.lock.write().await; // Write lock for delete

        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).map_err(ventoy_to_app_error)?;

            // Use recursive delete to handle directories
            image
                .remove_recursive(&path_to_delete)
                .map_err(ventoy_to_app_error)
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
    }
}

/// Convert VentoyError to AppError
fn ventoy_to_app_error(err: VentoyError) -> AppError {
    match err {
        VentoyError::Io(e) => AppError::Io(e),
        VentoyError::InvalidSize(s) => AppError::BadRequest(format!("Invalid size: {}", s)),
        VentoyError::SizeParseError(s) => {
            AppError::BadRequest(format!("Size parse error: {}", s))
        }
        VentoyError::FilesystemError(s) => {
            AppError::Internal(format!("Filesystem error: {}", s))
        }
        VentoyError::ImageError(s) => AppError::Internal(format!("Image error: {}", s)),
        VentoyError::FileNotFound(s) => AppError::NotFound(format!("File not found: {}", s)),
        VentoyError::ResourceNotFound(s) => {
            AppError::Internal(format!("Resource not found: {}", s))
        }
        VentoyError::PartitionError(s) => {
            AppError::Internal(format!("Partition error: {}", s))
        }
    }
}

/// Convert VentoyFileInfo to DriveFile
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
        modified: None, // Ventoy FileInfo doesn't include timestamps
    }
}

/// A writer that sends chunks to an async channel
///
/// This bridges the sync Write trait with async channels for streaming.
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

            self.buffer.extend_from_slice(&buf[written..written + to_copy]);
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
        // Flush any remaining data when the writer is dropped
        let _ = self.flush_buffer();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::OnceLock;
    use tempfile::TempDir;

    /// Path to ventoy resources directory
    static RESOURCE_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ventoy-img-rs/resources"
    );

    /// Initialize ventoy resources once
    fn init_ventoy_resources() -> bool {
        static INIT: OnceLock<bool> = OnceLock::new();
        *INIT.get_or_init(|| {
            let resource_path = std::path::Path::new(RESOURCE_DIR);

            // Decompress xz files if needed
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

            // Initialize resources
            if let Err(e) = ventoy_img::resources::init_resources(resource_path) {
                eprintln!("Failed to init ventoy resources: {}", e);
                return false;
            }

            true
        })
    }

    /// Decompress xz file using system command
    fn decompress_xz(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        let output = Command::new("xz")
            .args(&["-d", "-k", "-c", src.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("xz decompress failed: {}", String::from_utf8_lossy(&output.stderr)),
            ));
        }

        std::fs::write(dst, &output.stdout)?;
        Ok(())
    }

    /// Ensure resources are initialized, skip test if failed
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
        assert!(drive.exists());
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

        // Initialize drive
        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();

        // Write a test file
        let test_content = b"Hello, Ventoy!";
        let test_file_path = temp_dir.path().join("test.txt");
        std::fs::write(&test_file_path, test_content).unwrap();

        // Add file to drive using ventoy-img directly
        let path = drive.path().clone();
        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).unwrap();
            image.add_file(&test_file_path).unwrap();
        })
        .await
        .unwrap();

        // Read file from drive
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

        // Initialize drive
        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();

        // Create a directory
        drive.mkdir("/mydir").await.unwrap();

        // Write a test file
        let test_content = b"Test file content for info check";
        let test_file_path = temp_dir.path().join("info_test.txt");
        std::fs::write(&test_file_path, test_content).unwrap();

        // Add file to drive
        let path = drive.path().clone();
        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).unwrap();
            image.add_file(&test_file_path).unwrap();
        })
        .await
        .unwrap();

        // Test get_file_info for file
        let file_info = drive.get_file_info("/info_test.txt").await.unwrap();
        assert!(file_info.is_some());
        let file_info = file_info.unwrap();
        assert_eq!(file_info.name, "info_test.txt");
        assert_eq!(file_info.size, test_content.len() as u64);
        assert!(!file_info.is_dir);

        // Test get_file_info for directory
        let dir_info = drive.get_file_info("/mydir").await.unwrap();
        assert!(dir_info.is_some());
        let dir_info = dir_info.unwrap();
        assert_eq!(dir_info.name, "mydir");
        assert!(dir_info.is_dir);

        // Test get_file_info for non-existent file
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

        // Initialize drive
        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();

        // Create test data that spans multiple chunks (>64KB)
        let test_size = 200 * 1024; // 200 KB
        let test_content: Vec<u8> = (0..test_size).map(|i| (i % 256) as u8).collect();
        let test_file_path = temp_dir.path().join("large_file.bin");
        std::fs::write(&test_file_path, &test_content).unwrap();

        // Add file to drive
        let path = drive.path().clone();
        let file_path_clone = test_file_path.clone();
        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).unwrap();
            image.add_file(&file_path_clone).unwrap();
        })
        .await
        .unwrap();

        // Stream read the file
        let (file_size, mut rx) = drive.read_file_stream("/large_file.bin").await.unwrap();
        assert_eq!(file_size, test_size as u64);

        // Collect all chunks
        let mut received_data = Vec::new();
        while let Some(chunk_result) = rx.recv().await {
            let chunk = chunk_result.expect("Chunk should not be an error");
            received_data.extend_from_slice(&chunk);
        }

        // Verify data matches
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

        // Initialize drive
        drive.init(MIN_DRIVE_SIZE_MB).await.unwrap();

        // Create a small test file
        let test_content = b"Small file for streaming test";
        let test_file_path = temp_dir.path().join("small.txt");
        std::fs::write(&test_file_path, test_content).unwrap();

        // Add file to drive
        let path = drive.path().clone();
        tokio::task::spawn_blocking(move || {
            let mut image = VentoyImage::open(&path).unwrap();
            image.add_file(&test_file_path).unwrap();
        })
        .await
        .unwrap();

        // Stream read the file
        let (file_size, mut rx) = drive.read_file_stream("/small.txt").await.unwrap();
        assert_eq!(file_size, test_content.len() as u64);

        // Collect all chunks
        let mut received_data = Vec::new();
        while let Some(chunk_result) = rx.recv().await {
            let chunk = chunk_result.expect("Chunk should not be an error");
            received_data.extend_from_slice(&chunk);
        }

        // Verify data matches
        assert_eq!(received_data.as_slice(), test_content);
    }
}
