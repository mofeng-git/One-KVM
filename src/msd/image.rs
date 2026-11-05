use futures::StreamExt;
use std::fs;
#[cfg(test)]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use super::types::ImageInfo;
use crate::error::{AppError, MsdErrorCode, Result};

const MAX_IMAGE_SIZE: u64 = 32 * 1024 * 1024 * 1024;

const PROGRESS_THROTTLE_MS: u64 = 200;

const PROGRESS_THROTTLE_BYTES: u64 = 512 * 1024;

pub struct ImageManager {
    images_path: PathBuf,
}

impl ImageManager {
    pub fn new(images_path: PathBuf) -> Self {
        Self { images_path }
    }

    pub fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.images_path)
            .map_err(|error| storage_io_error("create images directory", error))?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<ImageInfo>> {
        self.ensure_dir()?;

        let mut images = Vec::new();

        for entry in fs::read_dir(&self.images_path)
            .map_err(|error| storage_io_error("read images directory", error))?
        {
            let entry = entry.map_err(|error| storage_io_error("read image entry", error))?;

            let path = entry.path();
            if path.is_file() {
                if let Some(info) = self.get_image_info(&path) {
                    images.push(info);
                }
            }
        }

        images.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(images)
    }

    fn get_image_info(&self, path: &Path) -> Option<ImageInfo> {
        let metadata = fs::metadata(path).ok()?;
        let name = path.file_name()?.to_string_lossy().to_string();

        let id = stable_image_id_from_filename(&name);

        let created_at = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                OffsetDateTime::from_unix_timestamp(d.as_secs() as i64)
                    .unwrap_or_else(|_| OffsetDateTime::now_utc())
            })
            .unwrap_or_else(OffsetDateTime::now_utc);

        Some(ImageInfo {
            id,
            name,
            path: path.to_path_buf(),
            size: metadata.len(),
            created_at,
        })
    }

    pub fn get(&self, id: &str) -> Result<ImageInfo> {
        for image in self.list()? {
            if image.id == id {
                return Ok(image);
            }
        }
        Err(MsdErrorCode::MsdResourceNotFound.into())
    }

    pub fn get_by_name(&self, name: &str) -> Result<ImageInfo> {
        let path = self.images_path.join(name);
        self.get_image_info(&path)
            .ok_or_else(|| MsdErrorCode::MsdResourceNotFound.into())
    }

    #[cfg(test)]
    fn create(&self, name: &str, data: &[u8]) -> Result<ImageInfo> {
        self.ensure_dir()?;

        let name = sanitize_filename(name);
        if name.is_empty() {
            return Err(MsdErrorCode::MsdInvalidRequest.into());
        }

        if data.len() as u64 > MAX_IMAGE_SIZE {
            return Err(MsdErrorCode::MsdImageTooLarge.into());
        }

        let path = self.images_path.join(&name);
        if path.exists() {
            return Err(MsdErrorCode::MsdResourceAlreadyExists.into());
        }

        let mut file =
            fs::File::create(&path).map_err(|error| storage_io_error("create image", error))?;

        file.write_all(data).map_err(|error| {
            let _ = fs::remove_file(&path);
            storage_io_error("write image", error)
        })?;

        info!("Created image: {} ({} bytes)", name, data.len());

        self.get_by_name(&name)
    }

    pub async fn create_from_multipart_field(
        &self,
        name: &str,
        mut field: axum::extract::multipart::Field<'_>,
    ) -> Result<ImageInfo> {
        self.ensure_dir()?;

        let name = sanitize_filename(name);
        if name.is_empty() {
            return Err(MsdErrorCode::MsdInvalidRequest.into());
        }

        let temp_name = format!(".upload_{}", uuid::Uuid::new_v4());
        let temp_path = self.images_path.join(&temp_name);
        let final_path = self.images_path.join(&name);

        if final_path.exists() {
            return Err(MsdErrorCode::MsdResourceAlreadyExists.into());
        }

        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|error| storage_io_error("create image upload", error))?;

        let mut bytes_written: u64 = 0;

        while let Some(chunk) = field.chunk().await.map_err(|error| {
            warn!(%error, "Failed to read MSD image upload chunk");
            AppError::from(MsdErrorCode::MsdOperationFailed)
        })? {
            bytes_written += chunk.len() as u64;
            if bytes_written > MAX_IMAGE_SIZE {
                drop(file);
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(MsdErrorCode::MsdImageTooLarge.into());
            }

            file.write_all(&chunk)
                .await
                .map_err(|error| storage_io_error("write image upload", error))?;
        }

        file.flush()
            .await
            .map_err(|error| storage_io_error("flush image upload", error))?;
        drop(file);

        tokio::fs::rename(&temp_path, &final_path)
            .await
            .map_err(|error| {
                let _ = std::fs::remove_file(&temp_path);
                storage_io_error("commit image upload", error)
            })?;

        info!(
            "Created image (streaming): {} ({} bytes)",
            name, bytes_written
        );

        self.get_by_name(&name)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let image = self.get(id)?;

        fs::remove_file(&image.path).map_err(|error| storage_io_error("delete image", error))?;

        info!("Deleted image: {}", image.name);
        Ok(())
    }

    pub async fn download_from_url<F>(
        &self,
        url: &str,
        filename: Option<String>,
        progress_callback: F,
    ) -> Result<ImageInfo>
    where
        F: Fn(u64, Option<u64>) + Send + 'static,
    {
        self.ensure_dir()?;

        let parsed_url =
            reqwest::Url::parse(url).map_err(|_| AppError::from(MsdErrorCode::MsdInvalidUrl))?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(MsdErrorCode::MsdInvalidUrl.into());
        }

        info!("Starting download from: {}", url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600))
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|error| remote_download_error("create HTTP client", error))?;

        let head_response = client
            .head(url)
            .send()
            .await
            .map_err(|error| remote_download_error("send HEAD request", error))?;

        if !head_response.status().is_success() {
            warn!(status = %head_response.status(), "MSD image HEAD request failed");
            return Err(MsdErrorCode::MsdRemoteDownloadFailed.into());
        }

        let total_size = head_response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        if let Some(size) = total_size {
            if size > MAX_IMAGE_SIZE {
                return Err(MsdErrorCode::MsdImageTooLarge.into());
            }
        }

        let final_filename = if let Some(name) = filename {
            sanitize_filename(&name)
        } else {
            let from_header = head_response
                .headers()
                .get(reqwest::header::CONTENT_DISPOSITION)
                .and_then(|v| v.to_str().ok())
                .and_then(extract_filename_from_content_disposition);

            if let Some(name) = from_header {
                sanitize_filename(&name)
            } else {
                let path = parsed_url.path();
                let name = path.rsplit('/').next().unwrap_or("download");
                let name = urlencoding::decode(name).unwrap_or_else(|_| name.into());
                sanitize_filename(&name)
            }
        };

        if final_filename.is_empty() {
            return Err(MsdErrorCode::MsdInvalidRequest.into());
        }

        let final_path = self.images_path.join(&final_filename);
        if final_path.exists() {
            return Err(MsdErrorCode::MsdResourceAlreadyExists.into());
        }

        let temp_filename = format!(".download_{}", uuid::Uuid::new_v4());
        let temp_path = self.images_path.join(&temp_filename);

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| remote_download_error("send GET request", error))?;

        if !response.status().is_success() {
            warn!(status = %response.status(), "MSD image GET request failed");
            return Err(MsdErrorCode::MsdRemoteDownloadFailed.into());
        }

        let content_length = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .or(total_size);

        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|error| storage_io_error("create image download", error))?;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_report_time = Instant::now();
        let mut last_reported_bytes: u64 = 0;
        let throttle_interval = Duration::from_millis(PROGRESS_THROTTLE_MS);

        progress_callback(0, content_length);

        while let Some(chunk_result) = stream.next().await {
            let chunk =
                chunk_result.map_err(|error| remote_download_error("read response body", error))?;

            file.write_all(&chunk).await.map_err(|error| {
                let _ = std::fs::remove_file(&temp_path);
                storage_io_error("write image download", error)
            })?;

            downloaded += chunk.len() as u64;

            let now = Instant::now();
            let time_elapsed = now.duration_since(last_report_time) >= throttle_interval;
            let bytes_elapsed = downloaded - last_reported_bytes >= PROGRESS_THROTTLE_BYTES;

            if time_elapsed || bytes_elapsed {
                progress_callback(downloaded, content_length);
                last_report_time = now;
                last_reported_bytes = downloaded;
            }
        }

        if downloaded != last_reported_bytes {
            progress_callback(downloaded, content_length);
        }

        file.flush()
            .await
            .map_err(|error| storage_io_error("flush image download", error))?;
        drop(file);

        let metadata = tokio::fs::metadata(&temp_path)
            .await
            .map_err(|error| storage_io_error("read downloaded image metadata", error))?;

        if let Some(expected) = content_length {
            if metadata.len() != expected {
                let _ = tokio::fs::remove_file(&temp_path).await;
                warn!(
                    actual = metadata.len(),
                    expected, "MSD image download was incomplete"
                );
                return Err(MsdErrorCode::MsdDownloadIncomplete.into());
            }
        }

        tokio::fs::rename(&temp_path, &final_path)
            .await
            .map_err(|error| {
                let _ = std::fs::remove_file(&temp_path);
                storage_io_error("commit image download", error)
            })?;

        info!(
            "Download complete: {} ({} bytes)",
            final_filename,
            metadata.len()
        );

        self.get_by_name(&final_filename)
    }
}

fn storage_io_error(operation: &'static str, error: std::io::Error) -> AppError {
    warn!(operation, %error, "MSD storage operation failed");
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

fn remote_download_error(operation: &'static str, error: reqwest::Error) -> AppError {
    warn!(operation, %error, "MSD remote download failed");
    MsdErrorCode::MsdRemoteDownloadFailed.into()
}

fn stable_image_id_from_filename(name: &str) -> String {
    let mut hash: u64 = 0;
    for (i, byte) in name.bytes().enumerate() {
        hash = hash.wrapping_add((byte as u64).wrapping_mul((i as u64).wrapping_add(1)));
        hash = hash.wrapping_mul(31);
    }
    format!("{:x}", hash)
}

fn sanitize_filename(name: &str) -> String {
    let name = name.trim();
    let name = name.replace(['/', '\\', '\0', ':', '*', '?', '"', '<', '>', '|'], "_");

    let name = name.trim_start_matches('.');

    if name.len() > 255 {
        name[..255].to_string()
    } else {
        name.to_string()
    }
}

fn extract_filename_from_content_disposition(header: &str) -> Option<String> {
    if let Some(pos) = header.find("filename*=") {
        let start = pos + 10;
        let value = &header[start..];
        if let Some(quote_start) = value.find("''") {
            let encoded = value[quote_start + 2..].split(';').next()?;
            let decoded = urlencoding::decode(encoded.trim()).ok()?;
            let name = decoded.trim_matches('"').to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    if let Some(pos) = header.find("filename=") {
        let start = pos + 9;
        let value = &header[start..];
        let name = value.split(';').next()?;
        let name = name.trim().trim_matches('"').to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test.iso"), "test.iso");
        assert_eq!(sanitize_filename("../test.iso"), "_test.iso");
        assert_eq!(sanitize_filename("test/file.iso"), "test_file.iso");
        assert_eq!(sanitize_filename(".hidden.iso"), "hidden.iso");
    }

    #[test]
    fn test_image_manager_list_empty() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ImageManager::new(temp_dir.path().to_path_buf());

        let images = manager.list().unwrap();
        assert!(images.is_empty());
    }

    #[test]
    fn test_image_manager_create() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ImageManager::new(temp_dir.path().to_path_buf());

        let data = vec![0u8; 1024];
        let image = manager.create("test.iso", &data).unwrap();

        assert_eq!(image.name, "test.iso");
        assert_eq!(image.size, 1024);
    }

    #[test]
    fn test_image_manager_delete() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ImageManager::new(temp_dir.path().to_path_buf());

        let data = vec![0u8; 1024];
        let image = manager.create("test.iso", &data).unwrap();

        manager.delete(&image.id).unwrap();

        assert!(manager.list().unwrap().is_empty());
    }

    #[test]
    fn classifies_storage_io_errors() {
        for (errno, expected) in [
            (libc::EFBIG, MsdErrorCode::MsdImageTooLarge),
            (libc::ENOSPC, MsdErrorCode::MsdStorageFull),
            (libc::EROFS, MsdErrorCode::MsdStorageReadOnly),
            (libc::EACCES, MsdErrorCode::MsdStoragePermissionDenied),
            (libc::EPERM, MsdErrorCode::MsdStoragePermissionDenied),
        ] {
            let error = storage_io_error("test", std::io::Error::from_raw_os_error(errno));
            assert!(matches!(error, AppError::Msd(error) if error.code() == expected));
        }
    }
}
