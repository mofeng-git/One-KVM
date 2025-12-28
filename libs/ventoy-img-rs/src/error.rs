//! Error types for ventoy-img

use thiserror::Error;

#[derive(Error, Debug)]
pub enum VentoyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid image size: {0}. Minimum is 64MB")]
    InvalidSize(String),

    #[error("Failed to parse size string: {0}")]
    SizeParseError(String),

    #[error("Partition error: {0}")]
    PartitionError(String),

    #[error("Filesystem error: {0}")]
    FilesystemError(String),

    #[error("Image not found or invalid: {0}")]
    ImageError(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),
}

pub type Result<T> = std::result::Result<T, VentoyError>;
