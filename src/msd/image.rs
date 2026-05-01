use futures::StreamExt;
use std::fs;
#[cfg(test)]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use tracing::info;

use super::types::ImageInfo;
use crate::error::{AppError, Result};

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
            .map_err(|e| AppError::Internal(format!("Failed to create images directory: {}", e)))?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<ImageInfo>> {
        self.ensure_dir()?;

        let mut images = Vec::new();

        for entry in fs::read_dir(&self.images_path)
            .map_err(|e| AppError::Internal(format!("Failed to read images directory: {}", e)))?
        {
            let entry = entry.map_err(|e| {
                AppError::Internal(format!("Failed to read directory entry: {}", e))
            })?;

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
        Err(AppError::NotFound(format!("Image not found: {}", id)))
    }

    pub fn get_by_name(&self, name: &str) -> Result<ImageInfo> {
        let path = self.images_path.join(name);
        self.get_image_info(&path)
            .ok_or_else(|| AppError::NotFound(format!("Image not found: {}", name)))
    }

    #[cfg(test)]
    fn create(&self, name: &str, data: &[u8]) -> Result<ImageInfo> {
        self.ensure_dir()?;

        let name = sanitize_filename(name);
        if name.is_empty() {
            return Err(AppError::Internal("Invalid filename".to_string()));
        }

        if data.len() as u64 > MAX_IMAGE_SIZE {
            return Err(AppError::Internal(format!(
                "Image too large. Maximum size: {} GB",
                MAX_IMAGE_SIZE / 1024 / 1024 / 1024
            )));
        }

        let path = self.images_path.join(&name);
        if path.exists() {
            return Err(AppError::Internal(format!(
                "Image already exists: {}",
                name
            )));
        }

        let mut file = fs::File::create(&path)
            .map_err(|e| AppError::Internal(format!("Failed to create image file: {}", e)))?;

        file.write_all(data).map_err(|e| {
            let _ = fs::remove_file(&path);
            AppError::Internal(format!("Failed to write image data: {}", e))
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
            return Err(AppError::Internal("Invalid filename".to_string()));
        }

        let temp_name = format!(".upload_{}", uuid::Uuid::new_v4());
        let temp_path = self.images_path.join(&temp_name);
        let final_path = self.images_path.join(&name);

        if final_path.exists() {
            return Err(AppError::Internal(format!(
                "Image already exists: {}",
                name
            )));
        }

        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create temp file: {}", e)))?;

        let mut bytes_written: u64 = 0;

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read upload chunk: {}", e)))?
        {
            bytes_written += chunk.len() as u64;
            if bytes_written > MAX_IMAGE_SIZE {
                drop(file);
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(AppError::Internal(format!(
                    "Image too large. Maximum size: {} GB",
                    MAX_IMAGE_SIZE / 1024 / 1024 / 1024
                )));
            }

            file.write_all(&chunk)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to write chunk: {}", e)))?;
        }

        file.flush()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to flush file: {}", e)))?;
        drop(file);

        tokio::fs::rename(&temp_path, &final_path)
            .await
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp_path);
                AppError::Internal(format!("Failed to rename temp file: {}", e))
            })?;

        info!(
            "Created image (streaming): {} ({} bytes)",
            name, bytes_written
        );

        self.get_by_name(&name)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let image = self.get(id)?;

        fs::remove_file(&image.path)
            .map_err(|e| AppError::Internal(format!("Failed to delete image: {}", e)))?;

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

        let parsed_url = reqwest::Url::parse(url)
            .map_err(|e| AppError::BadRequest(format!("Invalid URL: {}", e)))?;

        info!("Starting download from: {}", url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600))
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        let head_response = client
            .head(url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to connect: {}", e)))?;

        if !head_response.status().is_success() {
            return Err(AppError::Internal(format!(
                "Server returned error: {}",
                head_response.status()
            )));
        }

        let total_size = head_response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        if let Some(size) = total_size {
            if size > MAX_IMAGE_SIZE {
                return Err(AppError::BadRequest(format!(
                    "File too large: {} bytes (max {} GB)",
                    size,
                    MAX_IMAGE_SIZE / 1024 / 1024 / 1024
                )));
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
            return Err(AppError::BadRequest(
                "Could not determine filename".to_string(),
            ));
        }

        let final_path = self.images_path.join(&final_filename);
        if final_path.exists() {
            return Err(AppError::BadRequest(format!(
                "Image already exists: {}",
                final_filename
            )));
        }

        let temp_filename = format!(".download_{}", uuid::Uuid::new_v4());
        let temp_path = self.images_path.join(&temp_filename);

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Download failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal(format!(
                "Download failed: HTTP {}",
                response.status()
            )));
        }

        let content_length = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .or(total_size);

        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create temp file: {}", e)))?;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_report_time = Instant::now();
        let mut last_reported_bytes: u64 = 0;
        let throttle_interval = Duration::from_millis(PROGRESS_THROTTLE_MS);

        progress_callback(0, content_length);

        while let Some(chunk_result) = stream.next().await {
            let chunk =
                chunk_result.map_err(|e| AppError::Internal(format!("Download error: {}", e)))?;

            file.write_all(&chunk).await.map_err(|e| {
                let _ = std::fs::remove_file(&temp_path);
                AppError::Internal(format!("Failed to write data: {}", e))
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
            .map_err(|e| AppError::Internal(format!("Failed to flush file: {}", e)))?;
        drop(file);

        let metadata = tokio::fs::metadata(&temp_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read file metadata: {}", e)))?;

        if let Some(expected) = content_length {
            if metadata.len() != expected {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(AppError::Internal(format!(
                    "Download incomplete: got {} bytes, expected {}",
                    metadata.len(),
                    expected
                )));
            }
        }

        tokio::fs::rename(&temp_path, &final_path)
            .await
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp_path);
                AppError::Internal(format!("Failed to move file: {}", e))
            })?;

        info!(
            "Download complete: {} ({} bytes)",
            final_filename,
            metadata.len()
        );

        self.get_by_name(&final_filename)
    }

    pub fn images_path(&self) -> &PathBuf {
        &self.images_path
    }
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
}
