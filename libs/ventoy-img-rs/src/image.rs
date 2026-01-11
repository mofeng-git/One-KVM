//! Ventoy image creation and management

use crate::error::{Result, VentoyError};
use crate::exfat::{format_exfat, ExfatFs, FileInfo};
use crate::partition::{
    parse_size, write_mbr_partition_table, PartitionLayout, SECTOR_SIZE, VENTOY_SIG_OFFSET,
};
use crate::resources::{get_boot_img, get_core_img, get_ventoy_disk_img, VENTOY_SIGNATURE};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Ventoy image builder and manager
pub struct VentoyImage {
    path: std::path::PathBuf,
    layout: PartitionLayout,
}

impl VentoyImage {
    /// Create a new Ventoy IMG file
    pub fn create(path: &Path, size_str: &str, label: &str) -> Result<Self> {
        let size = parse_size(size_str)?;
        let layout = PartitionLayout::calculate(size)?;

        println!(
            "[INFO] Creating {}MB image: {}",
            size / (1024 * 1024),
            path.display()
        );

        // Create sparse file
        let mut file = File::create(path)?;
        file.set_len(size)?;

        // Write boot code
        println!("[INFO] Writing boot code...");
        Self::write_boot_code(&mut file)?;

        // Write partition table
        println!("[INFO] Writing MBR partition table...");
        println!(
            "  Data partition: sector {} - {} ({} MB)",
            layout.data_start_sector,
            layout.data_start_sector + layout.data_size_sectors - 1,
            layout.data_size() / (1024 * 1024)
        );
        println!(
            "  EFI partition:  sector {} - {} (32 MB)",
            layout.efi_start_sector,
            layout.efi_start_sector + layout.efi_size_sectors - 1
        );
        write_mbr_partition_table(&mut file, &layout)?;

        // Write Ventoy signature
        println!("[INFO] Writing Ventoy signature...");
        Self::write_ventoy_signature(&mut file)?;

        // Write EFI partition
        println!("[INFO] Writing EFI partition...");
        Self::write_efi_partition(&mut file, &layout)?;

        // Format data partition as exFAT
        println!("[INFO] Formatting data partition as exFAT...");
        format_exfat(&mut file, layout.data_offset(), layout.data_size(), label)?;

        file.flush()?;

        println!("[INFO] Ventoy IMG created successfully!");

        Ok(Self {
            path: path.to_path_buf(),
            layout,
        })
    }

    /// Open an existing Ventoy IMG file
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;

        // Verify Ventoy signature
        let mut sig = [0u8; 16];
        file.seek(SeekFrom::Start(VENTOY_SIG_OFFSET))?;
        file.read_exact(&mut sig)?;

        if sig != VENTOY_SIGNATURE {
            return Err(VentoyError::ImageError(format!(
                "Invalid Ventoy signature in {}",
                path.display()
            )));
        }

        // Get file size and calculate layout
        let size = file.metadata()?.len();
        let layout = PartitionLayout::calculate(size)?;

        Ok(Self {
            path: path.to_path_buf(),
            layout,
        })
    }

    /// Write boot code (boot.img + core.img)
    fn write_boot_code(file: &mut File) -> Result<()> {
        // Write boot.img MBR code (first 440 bytes)
        let boot_img = get_boot_img()?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&boot_img[..440])?;

        // Write core.img (sector 1-2047)
        let core_img = get_core_img()?;
        file.seek(SeekFrom::Start(SECTOR_SIZE))?;

        let max_size = 2047 * SECTOR_SIZE as usize;
        let write_size = core_img.len().min(max_size);
        file.write_all(&core_img[..write_size])?;

        Ok(())
    }

    /// Write Ventoy signature
    fn write_ventoy_signature(file: &mut File) -> Result<()> {
        file.seek(SeekFrom::Start(VENTOY_SIG_OFFSET))?;
        file.write_all(&VENTOY_SIGNATURE)?;
        Ok(())
    }

    /// Write EFI partition content
    fn write_efi_partition(file: &mut File, layout: &PartitionLayout) -> Result<()> {
        let efi_img = get_ventoy_disk_img()?;

        file.seek(SeekFrom::Start(layout.efi_offset()))?;

        let max_size = (layout.efi_size_sectors * SECTOR_SIZE) as usize;
        let write_size = efi_img.len().min(max_size);
        file.write_all(&efi_img[..write_size])?;

        Ok(())
    }

    /// Get partition layout
    pub fn layout(&self) -> &PartitionLayout {
        &self.layout
    }

    /// List files in the data partition (root directory)
    pub fn list_files(&self) -> Result<Vec<FileInfo>> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.list_files()
    }

    /// List files in a specific directory
    pub fn list_files_at(&self, path: &str) -> Result<Vec<FileInfo>> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.list_files_at(path)
    }

    /// List all files recursively
    pub fn list_files_recursive(&self) -> Result<Vec<FileInfo>> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.list_files_recursive()
    }

    /// Add a file to the data partition (root directory)
    pub fn add_file(&mut self, src_path: &Path) -> Result<()> {
        let name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| VentoyError::FilesystemError("Invalid filename".to_string()))?;

        let mut fs = ExfatFs::open(&self.path, &self.layout)?;

        // Use streaming write for efficiency
        let mut src_file = File::open(src_path)?;
        let size = src_file.metadata()?.len();

        fs.write_file_from_reader(name, &mut src_file, size)
    }

    /// Add a file to the data partition with overwrite option
    pub fn add_file_overwrite(&mut self, src_path: &Path, overwrite: bool) -> Result<()> {
        let name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| VentoyError::FilesystemError("Invalid filename".to_string()))?;

        let mut fs = ExfatFs::open(&self.path, &self.layout)?;

        let mut src_file = File::open(src_path)?;
        let size = src_file.metadata()?.len();

        fs.write_file_from_reader_overwrite(name, &mut src_file, size, overwrite)
    }

    /// Add a file to a specific path in the data partition
    ///
    /// # Arguments
    /// * `src_path` - Source file path on the local filesystem
    /// * `dest_path` - Destination path in the image (e.g., "iso/linux/ubuntu.iso")
    /// * `create_parents` - If true, creates intermediate directories as needed
    /// * `overwrite` - If true, overwrites existing files
    pub fn add_file_to_path(
        &mut self,
        src_path: &Path,
        dest_path: &str,
        create_parents: bool,
        overwrite: bool,
    ) -> Result<()> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;

        let mut src_file = File::open(src_path)?;
        let size = src_file.metadata()?.len();

        fs.write_file_from_reader_path(dest_path, &mut src_file, size, create_parents, overwrite)
    }

    /// Create a directory in the data partition
    ///
    /// # Arguments
    /// * `path` - Directory path to create (e.g., "iso/linux")
    /// * `create_parents` - If true, creates intermediate directories (mkdir -p behavior)
    pub fn create_directory(&mut self, path: &str, create_parents: bool) -> Result<()> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.create_directory(path, create_parents)
    }

    /// Remove a file from the data partition (root directory)
    pub fn remove_file(&mut self, name: &str) -> Result<()> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.delete_file(name)
    }

    /// Remove a file or empty directory at a specific path
    pub fn remove_path(&mut self, path: &str) -> Result<()> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.delete_path(path)
    }

    /// Remove a file or directory recursively
    pub fn remove_recursive(&mut self, path: &str) -> Result<()> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.delete_recursive(path)
    }

    /// Read a file from the data partition
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.read_file_path(path)
    }

    /// Read a file from the data partition to a writer (streaming)
    ///
    /// This is the preferred method for large files as it doesn't load
    /// the entire file into memory.
    pub fn read_file_to_writer<W: std::io::Write>(
        &self,
        path: &str,
        writer: &mut W,
    ) -> Result<u64> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.read_file_path_to_writer(path, writer)
    }

    /// Get file information without reading the content
    ///
    /// Returns file size, name, and whether it's a directory.
    /// Returns None if the file doesn't exist.
    pub fn get_file_info(&self, path: &str) -> Result<Option<FileInfo>> {
        let mut fs = ExfatFs::open(&self.path, &self.layout)?;
        fs.get_file_info_path(path)
    }

    /// Get image path
    pub fn path(&self) -> &Path {
        &self.path
    }
}
