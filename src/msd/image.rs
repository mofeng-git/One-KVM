//! Image file manager
//!
//! Handles ISO/IMG image file operations:
//! - List available images
//! - Upload new images
//! - Delete images
//! - Metadata management
//! - Download from URL

use chrono::Utc;
use futures::StreamExt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tracing::info;

use super::types::ImageInfo;
use crate::error::{AppError, Result};

/// Maximum image size (32 GB)
const MAX_IMAGE_SIZE: u64 = 32 * 1024 * 1024 * 1024;

/// Progress report throttle interval (milliseconds)
const PROGRESS_THROTTLE_MS: u64 = 200;

/// Progress report throttle bytes threshold (512 KB)
const PROGRESS_THROTTLE_BYTES: u64 = 512 * 1024;

/// Image Manager
pub struct ImageManager {
    /// Images storage directory
    images_path: PathBuf,
}

impl ImageManager {
    /// Create a new image manager
    pub fn new(images_path: PathBuf) -> Self {
        Self { images_path }
    }

    /// Ensure images directory exists
    pub fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.images_path).map_err(|e| {
            AppError::Internal(format!("Failed to create images directory: {}", e))
        })?;
        Ok(())
    }

    /// List all available images
    pub fn list(&self) -> Result<Vec<ImageInfo>> {
        self.ensure_dir()?;

        let mut images = Vec::new();

        for entry in fs::read_dir(&self.images_path).map_err(|e| {
            AppError::Internal(format!("Failed to read images directory: {}", e))
        })? {
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

        // Sort by creation time (newest first)
        images.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(images)
    }

    /// Get image info from path
    fn get_image_info(&self, path: &Path) -> Option<ImageInfo> {
        let metadata = fs::metadata(path).ok()?;
        let name = path.file_name()?.to_string_lossy().to_string();

        // Use filename hash as ID (stable across restarts)
        let id = format!("{:x}", md5_hash(&name));

        let created_at = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                    .unwrap_or_else(|| Utc::now().into())
            })
            .unwrap_or_else(Utc::now);

        Some(ImageInfo {
            id,
            name,
            path: path.to_path_buf(),
            size: metadata.len(),
            created_at,
        })
    }

    /// Get image by ID
    pub fn get(&self, id: &str) -> Result<ImageInfo> {
        for image in self.list()? {
            if image.id == id {
                return Ok(image);
            }
        }
        Err(AppError::NotFound(format!("Image not found: {}", id)))
    }

    /// Get image by name
    pub fn get_by_name(&self, name: &str) -> Result<ImageInfo> {
        let path = self.images_path.join(name);
        self.get_image_info(&path)
            .ok_or_else(|| AppError::NotFound(format!("Image not found: {}", name)))
    }

    /// Create a new image from bytes
    pub fn create(&self, name: &str, data: &[u8]) -> Result<ImageInfo> {
        self.ensure_dir()?;

        // Validate name
        let name = sanitize_filename(name);
        if name.is_empty() {
            return Err(AppError::Internal("Invalid filename".to_string()));
        }

        // Check size
        if data.len() as u64 > MAX_IMAGE_SIZE {
            return Err(AppError::Internal(format!(
                "Image too large. Maximum size: {} GB",
                MAX_IMAGE_SIZE / 1024 / 1024 / 1024
            )));
        }

        // Write file
        let path = self.images_path.join(&name);
        if path.exists() {
            return Err(AppError::Internal(format!(
                "Image already exists: {}",
                name
            )));
        }

        let mut file = File::create(&path).map_err(|e| {
            AppError::Internal(format!("Failed to create image file: {}", e))
        })?;

        file.write_all(data).map_err(|e| {
            // Try to clean up on error
            let _ = fs::remove_file(&path);
            AppError::Internal(format!("Failed to write image data: {}", e))
        })?;

        info!("Created image: {} ({} bytes)", name, data.len());

        self.get_by_name(&name)
    }

    /// Create a new image from a file stream (for chunked uploads)
    pub fn create_from_stream<R: Read>(
        &self,
        name: &str,
        reader: &mut R,
        expected_size: Option<u64>,
    ) -> Result<ImageInfo> {
        self.ensure_dir()?;

        let name = sanitize_filename(name);
        if name.is_empty() {
            return Err(AppError::Internal("Invalid filename".to_string()));
        }

        if let Some(size) = expected_size {
            if size > MAX_IMAGE_SIZE {
                return Err(AppError::Internal(format!(
                    "Image too large. Maximum size: {} GB",
                    MAX_IMAGE_SIZE / 1024 / 1024 / 1024
                )));
            }
        }

        let path = self.images_path.join(&name);
        if path.exists() {
            return Err(AppError::Internal(format!(
                "Image already exists: {}",
                name
            )));
        }

        // Create file and copy data
        let mut file = File::create(&path).map_err(|e| {
            AppError::Internal(format!("Failed to create image file: {}", e))
        })?;

        let bytes_written = io::copy(reader, &mut file).map_err(|e| {
            let _ = fs::remove_file(&path);
            AppError::Internal(format!("Failed to write image data: {}", e))
        })?;

        info!("Created image: {} ({} bytes)", name, bytes_written);

        self.get_by_name(&name)
    }

    /// Create a new image from an async multipart field (streaming, memory-efficient)
    ///
    /// This method streams data directly to disk without buffering the entire file in memory,
    /// making it suitable for large files (multi-GB ISOs).
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

        // Use a temporary file during upload
        let temp_name = format!(".upload_{}", uuid::Uuid::new_v4());
        let temp_path = self.images_path.join(&temp_name);
        let final_path = self.images_path.join(&name);

        // Check if final file already exists
        if final_path.exists() {
            return Err(AppError::Internal(format!(
                "Image already exists: {}",
                name
            )));
        }

        // Create temp file
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create temp file: {}", e)))?;

        let mut bytes_written: u64 = 0;

        // Stream chunks directly to disk
        while let Some(chunk) = field.chunk().await.map_err(|e| {
            AppError::Internal(format!("Failed to read upload chunk: {}", e))
        })? {
            // Check size limit
            bytes_written += chunk.len() as u64;
            if bytes_written > MAX_IMAGE_SIZE {
                // Cleanup and return error
                drop(file);
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(AppError::Internal(format!(
                    "Image too large. Maximum size: {} GB",
                    MAX_IMAGE_SIZE / 1024 / 1024 / 1024
                )));
            }

            // Write chunk to file
            file.write_all(&chunk).await.map_err(|e| {
                AppError::Internal(format!("Failed to write chunk: {}", e))
            })?;
        }

        // Flush and close file
        file.flush().await.map_err(|e| {
            AppError::Internal(format!("Failed to flush file: {}", e))
        })?;
        drop(file);

        // Move temp file to final location
        tokio::fs::rename(&temp_path, &final_path)
            .await
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp_path);
                AppError::Internal(format!("Failed to rename temp file: {}", e))
            })?;

        info!("Created image (streaming): {} ({} bytes)", name, bytes_written);

        self.get_by_name(&name)
    }

    /// Delete an image by ID
    pub fn delete(&self, id: &str) -> Result<()> {
        let image = self.get(id)?;

        fs::remove_file(&image.path).map_err(|e| {
            AppError::Internal(format!("Failed to delete image: {}", e))
        })?;

        info!("Deleted image: {}", image.name);
        Ok(())
    }

    /// Delete an image by name
    pub fn delete_by_name(&self, name: &str) -> Result<()> {
        let path = self.images_path.join(name);

        if !path.exists() {
            return Err(AppError::NotFound(format!("Image not found: {}", name)));
        }

        fs::remove_file(&path).map_err(|e| {
            AppError::Internal(format!("Failed to delete image: {}", e))
        })?;

        info!("Deleted image: {}", name);
        Ok(())
    }

    /// Get total storage used
    pub fn used_space(&self) -> u64 {
        self.list()
            .map(|images| images.iter().map(|i| i.size).sum())
            .unwrap_or(0)
    }

    /// Check if storage has space for new image
    pub fn has_space(&self, size: u64) -> bool {
        // For now, just check against max size
        // In the future, could check disk space
        size <= MAX_IMAGE_SIZE
    }

    /// Download image from URL with progress callback
    ///
    /// # Arguments
    /// * `url` - The URL to download from
    /// * `filename` - Optional custom filename (extracted from URL or Content-Disposition if not provided)
    /// * `progress_callback` - Callback function called with (bytes_downloaded, total_bytes)
    ///
    /// # Returns
    /// * `Ok(ImageInfo)` - The downloaded image info
    /// * `Err(AppError)` - If download fails
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

        // Validate URL
        let parsed_url = reqwest::Url::parse(url)
            .map_err(|e| AppError::BadRequest(format!("Invalid URL: {}", e)))?;

        info!("Starting download from: {}", url);

        // Create HTTP client with timeout
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600)) // 1 hour timeout for large files
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        // Send HEAD request first to get content info
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

        // Check file size
        if let Some(size) = total_size {
            if size > MAX_IMAGE_SIZE {
                return Err(AppError::BadRequest(format!(
                    "File too large: {} bytes (max {} GB)",
                    size,
                    MAX_IMAGE_SIZE / 1024 / 1024 / 1024
                )));
            }
        }

        // Determine filename
        let final_filename = if let Some(name) = filename {
            sanitize_filename(&name)
        } else {
            // Try Content-Disposition header first
            let from_header = head_response
                .headers()
                .get(reqwest::header::CONTENT_DISPOSITION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| extract_filename_from_content_disposition(s));

            if let Some(name) = from_header {
                sanitize_filename(&name)
            } else {
                // Fall back to URL path
                let path = parsed_url.path();
                let name = path.rsplit('/').next().unwrap_or("download");
                let name = urlencoding::decode(name).unwrap_or_else(|_| name.into());
                sanitize_filename(&name)
            }
        };

        if final_filename.is_empty() {
            return Err(AppError::BadRequest("Could not determine filename".to_string()));
        }

        // Check if file already exists
        let final_path = self.images_path.join(&final_filename);
        if final_path.exists() {
            return Err(AppError::BadRequest(format!(
                "Image already exists: {}",
                final_filename
            )));
        }

        // Create temporary file for download
        let temp_filename = format!(".download_{}", uuid::Uuid::new_v4());
        let temp_path = self.images_path.join(&temp_filename);

        // Start actual download
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

        // Get actual content length from response (may differ from HEAD)
        let content_length = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .or(total_size);

        // Create temp file
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create temp file: {}", e)))?;

        // Stream download with progress (throttled)
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_report_time = Instant::now();
        let mut last_reported_bytes: u64 = 0;
        let throttle_interval = Duration::from_millis(PROGRESS_THROTTLE_MS);

        // Report initial progress
        progress_callback(0, content_length);

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| AppError::Internal(format!("Download error: {}", e)))?;

            file.write_all(&chunk)
                .await
                .map_err(|e| {
                    // Cleanup on error
                    let _ = std::fs::remove_file(&temp_path);
                    AppError::Internal(format!("Failed to write data: {}", e))
                })?;

            downloaded += chunk.len() as u64;

            // Throttled progress reporting: report if enough time or bytes have passed
            let now = Instant::now();
            let time_elapsed = now.duration_since(last_report_time) >= throttle_interval;
            let bytes_elapsed = downloaded - last_reported_bytes >= PROGRESS_THROTTLE_BYTES;

            if time_elapsed || bytes_elapsed {
                progress_callback(downloaded, content_length);
                last_report_time = now;
                last_reported_bytes = downloaded;
            }
        }

        // Always report final progress
        if downloaded != last_reported_bytes {
            progress_callback(downloaded, content_length);
        }

        // Ensure all data is flushed
        file.flush()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to flush file: {}", e)))?;
        drop(file);

        // Verify downloaded size
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

        // Move temp file to final location
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

        // Return image info
        self.get_by_name(&final_filename)
    }

    /// Get images storage path
    pub fn images_path(&self) -> &PathBuf {
        &self.images_path
    }
}

/// Simple hash function for generating stable IDs
fn md5_hash(s: &str) -> u64 {
    let mut hash: u64 = 0;
    for (i, byte) in s.bytes().enumerate() {
        hash = hash.wrapping_add((byte as u64).wrapping_mul((i as u64).wrapping_add(1)));
        hash = hash.wrapping_mul(31);
    }
    hash
}

/// Sanitize filename to prevent path traversal
fn sanitize_filename(name: &str) -> String {
    let name = name.trim();
    let name = name.replace(['/', '\\', '\0', ':', '*', '?', '"', '<', '>', '|'], "_");

    // Remove leading dots (hidden files)
    let name = name.trim_start_matches('.');

    // Limit length
    if name.len() > 255 {
        name[..255].to_string()
    } else {
        name.to_string()
    }
}

/// Extract filename from Content-Disposition header
fn extract_filename_from_content_disposition(header: &str) -> Option<String> {
    // Handle both:
    // Content-Disposition: attachment; filename="example.iso"
    // Content-Disposition: attachment; filename*=UTF-8''example.iso

    // Try filename* first (RFC 5987)
    if let Some(pos) = header.find("filename*=") {
        let start = pos + 10;
        let value = &header[start..];
        // Format: charset'language'value
        if let Some(quote_start) = value.find("''") {
            let encoded = value[quote_start + 2..].split(';').next()?;
            let decoded = urlencoding::decode(encoded.trim()).ok()?;
            let name = decoded.trim_matches('"').to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    // Try filename next
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
        assert_eq!(sanitize_filename("../test.iso"), "_test.iso"); // .. becomes empty after trim_start_matches('.')
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
