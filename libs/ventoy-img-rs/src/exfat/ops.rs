//! exFAT filesystem operations
//!
//! Complete exFAT file operations: read, write, delete files.
//! Supports streaming write for large files.
//! Supports subdirectories and file overwriting.

use crate::error::{Result, VentoyError};
use crate::exfat::unicode;
use crate::partition::PartitionLayout;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// FAT entry values
const FAT_ENTRY_FREE: u32 = 0x00000000;
const FAT_ENTRY_END_OF_CHAIN: u32 = 0xFFFFFFFF;

/// Directory entry types
const ENTRY_TYPE_END: u8 = 0x00;
const ENTRY_TYPE_FILE: u8 = 0x85;
const ENTRY_TYPE_STREAM: u8 = 0xC0;
const ENTRY_TYPE_FILE_NAME: u8 = 0xC1;
const ENTRY_TYPE_DELETED_FILE: u8 = 0x05;
const ENTRY_TYPE_DELETED_STREAM: u8 = 0x40;
const ENTRY_TYPE_DELETED_NAME: u8 = 0x41;

/// File attributes
const ATTR_DIRECTORY: u16 = 0x10;
const ATTR_ARCHIVE: u16 = 0x20;

/// FAT cache size (number of entries, 8192 entries = 32KB)
const FAT_CACHE_ENTRIES: usize = 8192;

/// FAT table segment cache for reducing disk I/O
struct FatCache {
    /// First cluster number in this cache segment
    start_cluster: u32,
    /// Cached FAT entries
    entries: Vec<u32>,
}

impl FatCache {
    /// Create a new empty FAT cache
    fn new() -> Self {
        Self {
            start_cluster: 0,
            entries: Vec::new(),
        }
    }

    /// Check if a cluster is in the cache
    fn contains(&self, cluster: u32) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        cluster >= self.start_cluster
            && cluster < self.start_cluster + self.entries.len() as u32
    }

    /// Get a FAT entry from cache (if present)
    fn get(&self, cluster: u32) -> Option<u32> {
        if self.contains(cluster) {
            let index = (cluster - self.start_cluster) as usize;
            Some(self.entries[index])
        } else {
            None
        }
    }

    /// Update a single entry in the cache (for write operations)
    fn update(&mut self, cluster: u32, value: u32) {
        if self.contains(cluster) {
            let index = (cluster - self.start_cluster) as usize;
            self.entries[index] = value;
        }
    }
}

// ==================== Path Utilities ====================

/// Parse a path into components
fn parse_path(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect()
}

/// File information
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub is_directory: bool,
    /// Path from root (for recursive listing)
    pub path: String,
}

/// Location of a file entry in the directory
#[derive(Debug, Clone)]
struct FileEntryLocation {
    /// Cluster containing the directory
    directory_cluster: u32,
    /// Byte offset within the cluster where the file entry starts
    entry_offset: u32,
    /// First cluster of file data
    first_cluster: u32,
    /// File size in bytes
    data_length: u64,
    /// Number of secondary entries (stream + name entries)
    secondary_count: u8,
    /// Whether this is a directory
    is_directory: bool,
}

/// Result of resolving a path
#[derive(Debug, Clone)]
struct ResolvedPath {
    /// The parent directory cluster (where the file/dir entry resides)
    parent_cluster: u32,
    /// The name of the target file/directory
    name: String,
    /// The location if the target exists
    location: Option<FileEntryLocation>,
}

/// exFAT filesystem with full read/write support
#[allow(dead_code)]
pub struct ExfatFs {
    file: File,
    partition_offset: u64,
    // Boot sector cached parameters
    bytes_per_sector: u32,
    sectors_per_cluster: u32,
    cluster_size: u32,
    fat_offset: u32,
    fat_length: u32,
    cluster_heap_offset: u32,
    cluster_count: u32,
    first_cluster_of_root: u32,
    // Performance caches
    /// FAT table segment cache
    fat_cache: FatCache,
    /// Allocation bitmap cache (loaded on first access)
    bitmap_cache: Option<Vec<u8>>,
    /// Whether the bitmap cache has been modified
    bitmap_dirty: bool,
}

impl ExfatFs {
    /// Open exFAT filesystem from image file
    pub fn open(path: &Path, layout: &PartitionLayout) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(VentoyError::Io)?;

        let partition_offset = layout.data_offset();

        // Read and parse boot sector
        let mut boot_sector = [0u8; 512];
        file.seek(SeekFrom::Start(partition_offset))?;
        file.read_exact(&mut boot_sector)?;

        // Verify exFAT signature
        if &boot_sector[3..11] != b"EXFAT   " {
            return Err(VentoyError::FilesystemError(
                "Invalid exFAT signature".to_string(),
            ));
        }

        // Parse boot sector fields
        let fat_offset = u32::from_le_bytes(boot_sector[80..84].try_into().unwrap());
        let fat_length = u32::from_le_bytes(boot_sector[84..88].try_into().unwrap());
        let cluster_heap_offset = u32::from_le_bytes(boot_sector[88..92].try_into().unwrap());
        let cluster_count = u32::from_le_bytes(boot_sector[92..96].try_into().unwrap());
        let first_cluster_of_root = u32::from_le_bytes(boot_sector[96..100].try_into().unwrap());
        let bytes_per_sector_shift = boot_sector[108];
        let sectors_per_cluster_shift = boot_sector[109];

        let bytes_per_sector = 1u32 << bytes_per_sector_shift;
        let sectors_per_cluster = 1u32 << sectors_per_cluster_shift;
        let cluster_size = bytes_per_sector * sectors_per_cluster;

        Ok(Self {
            file,
            partition_offset,
            bytes_per_sector,
            sectors_per_cluster,
            cluster_size,
            fat_offset,
            fat_length,
            cluster_heap_offset,
            cluster_count,
            first_cluster_of_root,
            // Initialize caches
            fat_cache: FatCache::new(),
            bitmap_cache: None,
            bitmap_dirty: false,
        })
    }

    // ==================== Cluster I/O Operations ====================

    /// Convert cluster number to absolute byte offset
    fn cluster_to_offset(&self, cluster: u32) -> u64 {
        // Clusters start at 2, so subtract 2 to get heap-relative index
        let cluster_index = (cluster - 2) as u64;
        self.partition_offset
            + self.cluster_heap_offset as u64 * self.bytes_per_sector as u64
            + cluster_index * self.cluster_size as u64
    }

    /// Read a cluster's data
    fn read_cluster(&mut self, cluster: u32) -> Result<Vec<u8>> {
        let offset = self.cluster_to_offset(cluster);
        self.file.seek(SeekFrom::Start(offset))?;
        let mut data = vec![0u8; self.cluster_size as usize];
        self.file.read_exact(&mut data)?;
        Ok(data)
    }

    /// Write data to a cluster
    fn write_cluster(&mut self, cluster: u32, data: &[u8]) -> Result<()> {
        if data.len() > self.cluster_size as usize {
            return Err(VentoyError::FilesystemError(
                "Data exceeds cluster size".to_string(),
            ));
        }
        let offset = self.cluster_to_offset(cluster);
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)?;
        // Pad with zeros if data is smaller than cluster
        if data.len() < self.cluster_size as usize {
            let padding = vec![0u8; self.cluster_size as usize - data.len()];
            self.file.write_all(&padding)?;
        }
        Ok(())
    }

    // ==================== FAT Table Operations ====================

    /// Get the byte offset of a FAT entry
    fn fat_entry_offset(&self, cluster: u32) -> u64 {
        self.partition_offset + self.fat_offset as u64 * self.bytes_per_sector as u64 + cluster as u64 * 4
    }

    /// Load a FAT segment into cache starting from the given cluster
    fn load_fat_segment(&mut self, start_cluster: u32) -> Result<()> {
        // Calculate how many entries we can read
        let max_cluster = self.cluster_count + 2;
        let entries_to_read = FAT_CACHE_ENTRIES.min((max_cluster - start_cluster) as usize);

        if entries_to_read == 0 {
            return Ok(());
        }

        // Read FAT entries in bulk (4 bytes per entry)
        let offset = self.fat_entry_offset(start_cluster);
        self.file.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; entries_to_read * 4];
        self.file.read_exact(&mut buffer)?;

        // Parse entries
        let mut entries = Vec::with_capacity(entries_to_read);
        for chunk in buffer.chunks_exact(4) {
            entries.push(u32::from_le_bytes(chunk.try_into().unwrap()));
        }

        self.fat_cache.start_cluster = start_cluster;
        self.fat_cache.entries = entries;

        Ok(())
    }

    /// Read a FAT entry (with caching)
    fn read_fat_entry(&mut self, cluster: u32) -> Result<u32> {
        // Try cache first
        if let Some(entry) = self.fat_cache.get(cluster) {
            return Ok(entry);
        }

        // Cache miss - load a new segment starting from this cluster
        self.load_fat_segment(cluster)?;

        // Should be in cache now
        self.fat_cache.get(cluster).ok_or_else(|| {
            VentoyError::FilesystemError(format!("Failed to cache FAT entry for cluster {}", cluster))
        })
    }

    /// Write a FAT entry (updates cache if present)
    fn write_fat_entry(&mut self, cluster: u32, value: u32) -> Result<()> {
        let offset = self.fat_entry_offset(cluster);
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&value.to_le_bytes())?;

        // Update cache if this entry is cached
        self.fat_cache.update(cluster, value);

        Ok(())
    }

    /// Read the entire cluster chain starting from a cluster
    fn read_cluster_chain(&mut self, first_cluster: u32) -> Result<Vec<u32>> {
        let mut chain = Vec::new();
        let mut current = first_cluster;

        while current >= 2 && current < 0xFFFFFFF8 {
            chain.push(current);
            current = self.read_fat_entry(current)?;
            // Safety limit to prevent infinite loops
            if chain.len() > self.cluster_count as usize {
                return Err(VentoyError::FilesystemError(
                    "FAT chain too long, possible corruption".to_string(),
                ));
            }
        }

        Ok(chain)
    }

    // ==================== Allocation Bitmap Operations ====================

    /// Read the allocation bitmap (with caching)
    fn read_bitmap(&mut self) -> Result<Vec<u8>> {
        if let Some(ref bitmap) = self.bitmap_cache {
            return Ok(bitmap.clone());
        }

        let bitmap = self.read_cluster(2)?;
        self.bitmap_cache = Some(bitmap.clone());
        Ok(bitmap)
    }

    /// Get a mutable reference to the cached bitmap, loading if necessary
    fn get_bitmap_mut(&mut self) -> Result<&mut Vec<u8>> {
        if self.bitmap_cache.is_none() {
            let bitmap = self.read_cluster(2)?;
            self.bitmap_cache = Some(bitmap);
        }
        Ok(self.bitmap_cache.as_mut().unwrap())
    }

    /// Write the allocation bitmap (with cache management)
    #[allow(dead_code)]
    fn write_bitmap(&mut self, bitmap: &[u8]) -> Result<()> {
        self.write_cluster(2, bitmap)?;
        self.bitmap_cache = Some(bitmap.to_vec());
        self.bitmap_dirty = false;
        Ok(())
    }

    /// Flush dirty bitmap to disk if needed
    #[allow(dead_code)]
    fn flush_bitmap(&mut self) -> Result<()> {
        if self.bitmap_dirty {
            if let Some(bitmap) = self.bitmap_cache.take() {
                self.write_cluster(2, &bitmap)?;
                self.bitmap_cache = Some(bitmap);
                self.bitmap_dirty = false;
            }
        }
        Ok(())
    }

    /// Check if a cluster is allocated
    fn is_cluster_allocated(bitmap: &[u8], cluster: u32) -> bool {
        let index = (cluster - 2) as usize;
        let byte_index = index / 8;
        let bit_index = index % 8;
        if byte_index >= bitmap.len() {
            return false;
        }
        (bitmap[byte_index] & (1 << bit_index)) != 0
    }

    /// Set cluster allocation status in bitmap
    fn set_cluster_allocated(bitmap: &mut [u8], cluster: u32, allocated: bool) {
        let index = (cluster - 2) as usize;
        let byte_index = index / 8;
        let bit_index = index % 8;
        if byte_index < bitmap.len() {
            if allocated {
                bitmap[byte_index] |= 1 << bit_index;
            } else {
                bitmap[byte_index] &= !(1 << bit_index);
            }
        }
    }

    /// Find free clusters
    fn find_free_clusters(&mut self, count: usize) -> Result<Vec<u32>> {
        let bitmap = self.read_bitmap()?;
        let mut free_clusters = Vec::with_capacity(count);

        // Start from cluster after root directory
        // (root is at first_cluster_of_root, which varies based on cluster size)
        let start_cluster = self.first_cluster_of_root + 1;
        for cluster in start_cluster..self.cluster_count + 2 {
            if !Self::is_cluster_allocated(&bitmap, cluster) {
                free_clusters.push(cluster);
                if free_clusters.len() >= count {
                    break;
                }
            }
        }

        if free_clusters.len() < count {
            return Err(VentoyError::FilesystemError(format!(
                "Not enough free space: need {} clusters, found {}",
                count,
                free_clusters.len()
            )));
        }

        Ok(free_clusters)
    }

    /// Allocate clusters and create a chain
    fn allocate_clusters(&mut self, count: usize) -> Result<u32> {
        if count == 0 {
            return Err(VentoyError::FilesystemError(
                "Cannot allocate 0 clusters".to_string(),
            ));
        }

        let clusters = self.find_free_clusters(count)?;
        let first_cluster = clusters[0];

        // Update bitmap using cache
        {
            let bitmap = self.get_bitmap_mut()?;
            for &cluster in &clusters {
                Self::set_cluster_allocated(bitmap, cluster, true);
            }
        }
        // Flush bitmap immediately for data integrity
        self.flush_bitmap_now()?;

        // Create FAT chain
        for i in 0..clusters.len() {
            let next = if i + 1 < clusters.len() {
                clusters[i + 1]
            } else {
                FAT_ENTRY_END_OF_CHAIN
            };
            self.write_fat_entry(clusters[i], next)?;
        }

        Ok(first_cluster)
    }

    /// Free a cluster chain
    fn free_cluster_chain(&mut self, first_cluster: u32) -> Result<()> {
        let chain = self.read_cluster_chain(first_cluster)?;

        // Update bitmap using cache
        {
            let bitmap = self.get_bitmap_mut()?;
            for &cluster in &chain {
                Self::set_cluster_allocated(bitmap, cluster, false);
            }
        }
        // Flush bitmap immediately for data integrity
        self.flush_bitmap_now()?;

        // Clear FAT entries and update cache
        for &cluster in &chain {
            self.write_fat_entry(cluster, FAT_ENTRY_FREE)?;
        }

        Ok(())
    }

    /// Flush bitmap to disk immediately
    fn flush_bitmap_now(&mut self) -> Result<()> {
        if let Some(bitmap) = self.bitmap_cache.take() {
            self.write_cluster(2, &bitmap)?;
            self.bitmap_cache = Some(bitmap);
        }
        Ok(())
    }

    /// Extend a cluster chain by appending one new cluster
    ///
    /// Returns the cluster number of the newly allocated cluster
    fn extend_cluster_chain(&mut self, first_cluster: u32) -> Result<u32> {
        // Find the last cluster in the chain
        let chain = self.read_cluster_chain(first_cluster)?;
        let last_cluster = *chain.last().ok_or_else(|| {
            VentoyError::FilesystemError("Empty cluster chain".to_string())
        })?;

        // Allocate one new cluster
        let new_cluster = self.allocate_clusters(1)?;

        // Link the last cluster to the new cluster
        self.write_fat_entry(last_cluster, new_cluster)?;

        // Initialize the new cluster with zeros
        let empty_cluster = vec![0u8; self.cluster_size as usize];
        self.write_cluster(new_cluster, &empty_cluster)?;

        Ok(new_cluster)
    }

    // ==================== Directory Entry Operations ====================

    /// Calculate name hash for exFAT (used in Stream Extension entry)
    ///
    /// Uses Unicode-aware uppercase conversion for proper international support.
    fn calculate_name_hash(name: &str) -> u16 {
        unicode::calculate_name_hash(name)
    }

    /// Calculate entry set checksum
    fn calculate_entry_set_checksum(entries: &[[u8; 32]]) -> u16 {
        let mut checksum: u16 = 0;
        for (entry_idx, entry) in entries.iter().enumerate() {
            for (byte_idx, &byte) in entry.iter().enumerate() {
                // Skip checksum field in first entry (bytes 2-3)
                if entry_idx == 0 && (byte_idx == 2 || byte_idx == 3) {
                    continue;
                }
                checksum = checksum.rotate_right(1).wrapping_add(byte as u16);
            }
        }
        checksum
    }

    /// Create file directory entries for a new file
    fn create_file_entries(name: &str, first_cluster: u32, size: u64, is_dir: bool) -> Vec<[u8; 32]> {
        let name_utf16: Vec<u16> = name.encode_utf16().collect();
        let name_entries_needed = (name_utf16.len() + 14) / 15; // 15 chars per name entry
        let secondary_count = 1 + name_entries_needed; // Stream + Name entries

        let mut entries = Vec::with_capacity(2 + name_entries_needed);

        // 1. File Directory Entry (0x85)
        let mut file_entry = [0u8; 32];
        file_entry[0] = ENTRY_TYPE_FILE;
        file_entry[1] = secondary_count as u8;
        // Checksum at bytes 2-3 (filled later)
        let attrs: u16 = if is_dir { ATTR_DIRECTORY } else { ATTR_ARCHIVE };
        file_entry[4..6].copy_from_slice(&attrs.to_le_bytes());
        // Timestamps (simplified - use current time)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);
        // DOS timestamp format (simplified)
        let dos_time = ((now / 2) & 0x1F) | (((now / 60) & 0x3F) << 5) | (((now / 3600) & 0x1F) << 11);
        let dos_date = 1 | (1 << 5) | ((45) << 9); // Jan 1, 2025
        file_entry[8..12].copy_from_slice(&(dos_date as u32 | ((dos_time as u32) << 16)).to_le_bytes());
        file_entry[12..16].copy_from_slice(&(dos_date as u32 | ((dos_time as u32) << 16)).to_le_bytes());
        file_entry[16..20].copy_from_slice(&(dos_date as u32 | ((dos_time as u32) << 16)).to_le_bytes());
        entries.push(file_entry);

        // 2. Stream Extension Entry (0xC0)
        let mut stream_entry = [0u8; 32];
        stream_entry[0] = ENTRY_TYPE_STREAM;
        stream_entry[1] = 0x03; // GeneralSecondaryFlags: AllocationPossible | NoFatChain (for contiguous)
        // For non-contiguous files, use 0x01
        if size > 0 {
            stream_entry[1] = 0x01; // AllocationPossible, use FAT chain
        }
        stream_entry[3] = name_utf16.len() as u8; // NameLength
        let name_hash = Self::calculate_name_hash(name);
        stream_entry[4..6].copy_from_slice(&name_hash.to_le_bytes());
        // ValidDataLength
        stream_entry[8..16].copy_from_slice(&size.to_le_bytes());
        // Reserved at 16-19
        stream_entry[20..24].copy_from_slice(&first_cluster.to_le_bytes());
        // DataLength
        stream_entry[24..32].copy_from_slice(&size.to_le_bytes());
        entries.push(stream_entry);

        // 3. File Name Entries (0xC1)
        let mut char_index = 0;
        for _ in 0..name_entries_needed {
            let mut name_entry = [0u8; 32];
            name_entry[0] = ENTRY_TYPE_FILE_NAME;
            name_entry[1] = 0; // GeneralSecondaryFlags

            for i in 0..15 {
                if char_index < name_utf16.len() {
                    let offset = 2 + i * 2;
                    name_entry[offset..offset + 2].copy_from_slice(&name_utf16[char_index].to_le_bytes());
                    char_index += 1;
                }
            }
            entries.push(name_entry);
        }

        // Calculate and set checksum
        let checksum = Self::calculate_entry_set_checksum(&entries);
        entries[0][2..4].copy_from_slice(&checksum.to_le_bytes());

        entries
    }

    /// Find a file entry in a specific directory cluster
    fn find_entry_in_directory(&mut self, dir_cluster: u32, name: &str) -> Result<Option<FileEntryLocation>> {
        let target_name_lower = name.to_lowercase();

        // Read all clusters in the directory chain
        let dir_clusters = self.read_cluster_chain(dir_cluster)?;

        for &cluster in &dir_clusters {
            let cluster_data = self.read_cluster(cluster)?;
            let mut i = 0;

            while i < cluster_data.len() {
                let entry_type = cluster_data[i];

                if entry_type == ENTRY_TYPE_END {
                    // End of directory - no more entries in any cluster
                    return Ok(None);
                }

                if entry_type == ENTRY_TYPE_FILE {
                    let secondary_count = cluster_data[i + 1] as usize;
                    let attrs = u16::from_le_bytes(cluster_data[i + 4..i + 6].try_into().unwrap());
                    let is_directory = (attrs & ATTR_DIRECTORY) != 0;
                    let mut file_name = String::new();
                    let mut first_cluster = 0u32;
                    let mut data_length = 0u64;

                    // Parse secondary entries
                    for j in 1..=secondary_count {
                        let entry_offset = i + j * 32;
                        if entry_offset + 32 > cluster_data.len() {
                            break;
                        }

                        let sec_type = cluster_data[entry_offset];
                        if sec_type == ENTRY_TYPE_STREAM {
                            first_cluster = u32::from_le_bytes(
                                cluster_data[entry_offset + 20..entry_offset + 24]
                                    .try_into()
                                    .unwrap(),
                            );
                            data_length = u64::from_le_bytes(
                                cluster_data[entry_offset + 24..entry_offset + 32]
                                    .try_into()
                                    .unwrap(),
                            );
                        } else if sec_type == ENTRY_TYPE_FILE_NAME {
                            let name_chars: Vec<u16> = (2..32)
                                .step_by(2)
                                .map(|k| {
                                    u16::from_le_bytes([
                                        cluster_data[entry_offset + k],
                                        cluster_data[entry_offset + k + 1],
                                    ])
                                })
                                .take_while(|&c| c != 0)
                                .collect();
                            file_name.extend(char::decode_utf16(name_chars).filter_map(|r| r.ok()));
                        }
                    }

                    if file_name.to_lowercase() == target_name_lower {
                        return Ok(Some(FileEntryLocation {
                            directory_cluster: cluster,
                            entry_offset: i as u32,
                            first_cluster,
                            data_length,
                            secondary_count: secondary_count as u8,
                            is_directory,
                        }));
                    }

                    i += (1 + secondary_count) * 32;
                } else {
                    i += 32;
                }
            }
        }

        Ok(None)
    }

    /// Find a file entry in the root directory (backward compatible)
    fn find_file_entry(&mut self, name: &str) -> Result<Option<FileEntryLocation>> {
        self.find_entry_in_directory(self.first_cluster_of_root, name)
    }

    /// Resolve a path to its parent directory cluster and target name
    ///
    /// Returns the parent directory cluster and the target name.
    /// If create_parents is true, creates intermediate directories as needed.
    fn resolve_path(&mut self, path: &str, create_parents: bool) -> Result<ResolvedPath> {
        let components = parse_path(path);

        if components.is_empty() {
            return Err(VentoyError::FilesystemError("Empty path".to_string()));
        }

        // Start from root
        let mut current_cluster = self.first_cluster_of_root;

        // Navigate through all but the last component (which is the target)
        for (idx, &component) in components.iter().take(components.len() - 1).enumerate() {
            match self.find_entry_in_directory(current_cluster, component)? {
                Some(entry) => {
                    if !entry.is_directory {
                        return Err(VentoyError::FilesystemError(format!(
                            "'{}' is not a directory",
                            component
                        )));
                    }
                    current_cluster = entry.first_cluster;
                }
                None => {
                    if create_parents {
                        // Create the intermediate directory
                        let new_cluster = self.create_directory_in(current_cluster, component)?;
                        current_cluster = new_cluster;
                    } else {
                        let partial_path = components[..=idx].join("/");
                        return Err(VentoyError::FilesystemError(format!(
                            "Directory '{}' not found",
                            partial_path
                        )));
                    }
                }
            }
        }

        let target_name = components.last().unwrap().to_string();
        let location = self.find_entry_in_directory(current_cluster, &target_name)?;

        Ok(ResolvedPath {
            parent_cluster: current_cluster,
            name: target_name,
            location,
        })
    }

    /// Find a free slot in a directory for new entries
    ///
    /// Returns (cluster, offset_within_cluster)
    ///
    /// If no free slot is found in existing clusters, this method will
    /// automatically extend the directory by allocating a new cluster.
    fn find_free_slot_in_directory(&mut self, dir_cluster: u32, entries_needed: usize) -> Result<(u32, u32)> {
        let dir_clusters = self.read_cluster_chain(dir_cluster)?;

        for &cluster in &dir_clusters {
            let cluster_data = self.read_cluster(cluster)?;
            let mut i = 0;
            let mut consecutive_free = 0;
            let mut slot_start = 0;

            while i < cluster_data.len() {
                let entry_type = cluster_data[i];

                if entry_type == ENTRY_TYPE_END || entry_type == 0x00
                    || entry_type == ENTRY_TYPE_DELETED_FILE
                    || entry_type == ENTRY_TYPE_DELETED_STREAM
                    || entry_type == ENTRY_TYPE_DELETED_NAME {
                    if consecutive_free == 0 {
                        slot_start = i;
                    }
                    consecutive_free += 1;
                    if consecutive_free >= entries_needed {
                        return Ok((cluster, slot_start as u32));
                    }
                } else if entry_type == ENTRY_TYPE_FILE {
                    let secondary_count = cluster_data[i + 1] as usize;
                    i += (1 + secondary_count) * 32;
                    consecutive_free = 0;
                    continue;
                } else {
                    consecutive_free = 0;
                }

                i += 32;
            }

            // Check if we have enough space at the end of this cluster
            if consecutive_free >= entries_needed {
                return Ok((cluster, slot_start as u32));
            }
        }

        // No space found in existing clusters - extend the directory
        let new_cluster = self.extend_cluster_chain(dir_cluster)?;

        // Clear any END markers in previous clusters
        // This is critical: when we extend a directory, we need to clear any END markers
        // that may exist in previous clusters, otherwise list_files will stop prematurely
        let dir_clusters_before = self.read_cluster_chain(dir_cluster)?;
        for &cluster in &dir_clusters_before[..dir_clusters_before.len()-1] { // Exclude the newly added cluster
            let mut cluster_data = self.read_cluster(cluster)?;

            // Scan for END markers and replace them with 0xFF (invalid entry, will be skipped)
            for i in (0..cluster_data.len()).step_by(32) {
                if cluster_data[i] == ENTRY_TYPE_END {
                    cluster_data[i] = 0xFF; // Invalid entry type
                }
            }

            self.write_cluster(cluster, &cluster_data)?;
        }

        // Return the first slot in the new cluster (offset 0)
        Ok((new_cluster, 0))
    }

    /// Find a free slot in the root directory for new entries (backward compatible)
    #[allow(dead_code)]
    fn find_free_directory_slot(&mut self, entries_needed: usize) -> Result<u32> {
        let (_, offset) = self.find_free_slot_in_directory(self.first_cluster_of_root, entries_needed)?;
        Ok(offset)
    }

    /// Create an entry in a specific directory
    fn create_entry_in_directory(&mut self, dir_cluster: u32, name: &str, first_cluster: u32, size: u64, is_dir: bool) -> Result<()> {
        let entries = Self::create_file_entries(name, first_cluster, size, is_dir);
        let (slot_cluster, slot_offset) = self.find_free_slot_in_directory(dir_cluster, entries.len())?;

        let mut cluster_data = self.read_cluster(slot_cluster)?;

        // Write entries to the slot
        for (i, entry) in entries.iter().enumerate() {
            let offset = slot_offset as usize + i * 32;
            cluster_data[offset..offset + 32].copy_from_slice(entry);
        }

        self.write_cluster(slot_cluster, &cluster_data)?;
        Ok(())
    }

    /// Create a file entry in the root directory (backward compatible)
    #[allow(dead_code)]
    fn create_file_entry(&mut self, name: &str, first_cluster: u32, size: u64) -> Result<()> {
        self.create_entry_in_directory(self.first_cluster_of_root, name, first_cluster, size, false)
    }

    /// Create a directory in a specific parent directory
    ///
    /// Returns the cluster number of the new directory
    fn create_directory_in(&mut self, parent_cluster: u32, name: &str) -> Result<u32> {
        // Validate name
        if name.is_empty() || name.len() > 255 {
            return Err(VentoyError::FilesystemError(
                "Invalid directory name length".to_string(),
            ));
        }

        // Check if already exists
        if self.find_entry_in_directory(parent_cluster, name)?.is_some() {
            return Err(VentoyError::FilesystemError(format!(
                "Entry '{}' already exists",
                name
            )));
        }

        // Allocate a cluster for the new directory
        let dir_cluster = self.allocate_clusters(1)?;

        // Initialize the directory cluster with zeros (empty directory)
        let empty_cluster = vec![0u8; self.cluster_size as usize];
        self.write_cluster(dir_cluster, &empty_cluster)?;

        // Create directory entry in parent
        self.create_entry_in_directory(parent_cluster, name, dir_cluster, 0, true)?;

        self.file.flush()?;
        Ok(dir_cluster)
    }

    /// Delete a file entry (mark as deleted)
    fn delete_file_entry(&mut self, location: &FileEntryLocation) -> Result<()> {
        let mut cluster_data = self.read_cluster(location.directory_cluster)?;
        let offset = location.entry_offset as usize;

        // Mark file entry as deleted
        cluster_data[offset] = ENTRY_TYPE_DELETED_FILE;

        // Mark secondary entries as deleted
        for i in 1..=location.secondary_count as usize {
            let entry_offset = offset + i * 32;
            if entry_offset < cluster_data.len() {
                let entry_type = cluster_data[entry_offset];
                cluster_data[entry_offset] = match entry_type {
                    ENTRY_TYPE_STREAM => ENTRY_TYPE_DELETED_STREAM,
                    ENTRY_TYPE_FILE_NAME => ENTRY_TYPE_DELETED_NAME,
                    _ => entry_type & 0x7F, // Clear in-use bit
                };
            }
        }

        self.write_cluster(location.directory_cluster, &cluster_data)?;
        Ok(())
    }

    // ==================== Public File Operations ====================

    /// List files in a specific directory cluster
    fn list_files_in_directory(&mut self, dir_cluster: u32, current_path: &str) -> Result<Vec<FileInfo>> {
        let dir_clusters = self.read_cluster_chain(dir_cluster)?;

        // Pre-allocate Vec based on estimated entries
        // Each directory entry is 32 bytes, and a file typically uses 3+ entries (file, stream, name)
        // So estimate ~96 bytes per file on average
        let estimated_entries = dir_clusters.len() * (self.cluster_size as usize / 96) + 1;
        let mut files = Vec::with_capacity(estimated_entries);

        for (cluster_idx, &cluster) in dir_clusters.iter().enumerate() {
            let cluster_data = self.read_cluster(cluster)?;
            let mut i = 0;

            while i < cluster_data.len() {
                let entry_type = cluster_data[i];

                // ENTRY_TYPE_END marks end of directory entries
                // But in extended directories, we might have empty clusters that start with 0x00
                // Only treat 0x00 as end marker if we're in the first cluster
                if entry_type == ENTRY_TYPE_END {
                    if cluster_idx == 0 {
                        return Ok(files);
                    }
                    // In non-first clusters, skip the empty entry and continue
                }

                if entry_type == ENTRY_TYPE_FILE {
                    let secondary_count = cluster_data[i + 1] as usize;
                    let attrs = u16::from_le_bytes(cluster_data[i + 4..i + 6].try_into().unwrap());
                    let is_directory = (attrs & ATTR_DIRECTORY) != 0;

                    let mut file_name = String::new();
                    let mut file_size = 0u64;

                    for j in 1..=secondary_count {
                        let entry_offset = i + j * 32;
                        if entry_offset + 32 > cluster_data.len() {
                            break;
                        }

                        let sec_type = cluster_data[entry_offset];
                        if sec_type == ENTRY_TYPE_STREAM {
                            file_size = u64::from_le_bytes(
                                cluster_data[entry_offset + 24..entry_offset + 32]
                                    .try_into()
                                    .unwrap(),
                            );
                        } else if sec_type == ENTRY_TYPE_FILE_NAME {
                            let name_chars: Vec<u16> = (2..32)
                                .step_by(2)
                                .map(|k| {
                                    u16::from_le_bytes([
                                        cluster_data[entry_offset + k],
                                        cluster_data[entry_offset + k + 1],
                                    ])
                                })
                                .take_while(|&c| c != 0)
                                .collect();
                            file_name.extend(char::decode_utf16(name_chars).filter_map(|r| r.ok()));
                        }
                    }

                    if !file_name.is_empty() {
                        let full_path = if current_path.is_empty() {
                            file_name.clone()
                        } else {
                            format!("{}/{}", current_path, file_name)
                        };
                        files.push(FileInfo {
                            name: file_name,
                            size: file_size,
                            is_directory,
                            path: full_path,
                        });
                    }

                    i += (1 + secondary_count) * 32;
                } else {
                    i += 32;
                }
            }
        }

        Ok(files)
    }

    /// List files in root directory
    pub fn list_files(&mut self) -> Result<Vec<FileInfo>> {
        self.list_files_in_directory(self.first_cluster_of_root, "")
    }

    /// List files in a specific directory path
    pub fn list_files_at(&mut self, path: &str) -> Result<Vec<FileInfo>> {
        if path.is_empty() || path == "/" {
            return self.list_files();
        }

        let resolved = self.resolve_path(path, false)?;
        match resolved.location {
            Some(loc) if loc.is_directory => {
                self.list_files_in_directory(loc.first_cluster, path.trim_matches('/'))
            }
            Some(_) => Err(VentoyError::FilesystemError(format!(
                "'{}' is not a directory",
                path
            ))),
            None => Err(VentoyError::FilesystemError(format!(
                "Directory '{}' not found",
                path
            ))),
        }
    }

    /// List all files recursively
    pub fn list_files_recursive(&mut self) -> Result<Vec<FileInfo>> {
        let mut all_files = Vec::new();
        let mut dirs_to_visit = vec![(self.first_cluster_of_root, String::new())];

        while let Some((dir_cluster, current_path)) = dirs_to_visit.pop() {
            let files = self.list_files_in_directory(dir_cluster, &current_path)?;
            for file in files {
                if file.is_directory {
                    // Get the directory's first cluster
                    if let Some(loc) = self.find_entry_in_directory(dir_cluster, &file.name)? {
                        dirs_to_visit.push((loc.first_cluster, file.path.clone()));
                    }
                }
                all_files.push(file);
            }
        }

        Ok(all_files)
    }

    /// Write file data to allocated clusters and create directory entry
    fn write_file_data_and_entry(&mut self, dir_cluster: u32, name: &str, data: &[u8]) -> Result<()> {
        // Calculate clusters needed
        let clusters_needed = if data.is_empty() {
            0
        } else {
            ((data.len() as u64 + self.cluster_size as u64 - 1) / self.cluster_size as u64) as usize
        };

        // Allocate clusters
        let first_cluster = if clusters_needed > 0 {
            let first = self.allocate_clusters(clusters_needed)?;

            // Write file data
            let chain = self.read_cluster_chain(first)?;
            let mut data_offset = 0;
            for &cluster in &chain {
                let chunk_size = (data.len() - data_offset).min(self.cluster_size as usize);
                let chunk = &data[data_offset..data_offset + chunk_size];
                self.write_cluster(cluster, chunk)?;
                data_offset += chunk_size;
            }

            first
        } else {
            0
        };

        // Create directory entry
        self.create_entry_in_directory(dir_cluster, name, first_cluster, data.len() as u64, false)?;

        self.file.flush()?;
        Ok(())
    }

    /// Write a file to the filesystem (root directory, no overwrite)
    pub fn write_file(&mut self, name: &str, data: &[u8]) -> Result<()> {
        // Validate filename
        if name.is_empty() || name.len() > 255 {
            return Err(VentoyError::FilesystemError(
                "Invalid filename length".to_string(),
            ));
        }

        // Check if file already exists
        if self.find_file_entry(name)?.is_some() {
            return Err(VentoyError::FilesystemError(format!(
                "File '{}' already exists",
                name
            )));
        }

        self.write_file_data_and_entry(self.first_cluster_of_root, name, data)
    }

    /// Write a file to the filesystem with overwrite option
    pub fn write_file_overwrite(&mut self, name: &str, data: &[u8], overwrite: bool) -> Result<()> {
        // Validate filename
        if name.is_empty() || name.len() > 255 {
            return Err(VentoyError::FilesystemError(
                "Invalid filename length".to_string(),
            ));
        }

        // Check if file already exists
        if self.find_file_entry(name)?.is_some() {
            if overwrite {
                self.delete_file(name)?;
            } else {
                return Err(VentoyError::FilesystemError(format!(
                    "File '{}' already exists",
                    name
                )));
            }
        }

        self.write_file_data_and_entry(self.first_cluster_of_root, name, data)
    }

    /// Write a file to a specific path
    ///
    /// Path can include directories, e.g., "iso/linux/ubuntu.iso"
    /// If create_parents is true, intermediate directories will be created.
    /// If overwrite is true, existing files will be replaced.
    pub fn write_file_path(&mut self, path: &str, data: &[u8], create_parents: bool, overwrite: bool) -> Result<()> {
        let resolved = self.resolve_path(path, create_parents)?;

        // Validate filename
        if resolved.name.is_empty() || resolved.name.len() > 255 {
            return Err(VentoyError::FilesystemError(
                "Invalid filename length".to_string(),
            ));
        }

        // Handle existing file
        if let Some(location) = resolved.location {
            if location.is_directory {
                return Err(VentoyError::FilesystemError(format!(
                    "'{}' is a directory",
                    path
                )));
            }
            if overwrite {
                // Delete existing file
                if location.first_cluster >= 2 {
                    self.free_cluster_chain(location.first_cluster)?;
                }
                self.delete_file_entry(&location)?;
            } else {
                return Err(VentoyError::FilesystemError(format!(
                    "File '{}' already exists",
                    path
                )));
            }
        }

        self.write_file_data_and_entry(resolved.parent_cluster, &resolved.name, data)
    }

    /// Read file data from a location
    fn read_file_from_location(&mut self, location: &FileEntryLocation) -> Result<Vec<u8>> {
        if location.data_length == 0 {
            return Ok(Vec::new());
        }

        let chain = self.read_cluster_chain(location.first_cluster)?;
        let mut data = Vec::with_capacity(location.data_length as usize);

        for &cluster in &chain {
            let cluster_data = self.read_cluster(cluster)?;
            let remaining = location.data_length as usize - data.len();
            let chunk_size = remaining.min(self.cluster_size as usize);
            data.extend_from_slice(&cluster_data[..chunk_size]);
        }

        Ok(data)
    }

    /// Read a file from the filesystem (root directory)
    pub fn read_file(&mut self, name: &str) -> Result<Vec<u8>> {
        let location = self.find_file_entry(name)?.ok_or_else(|| {
            VentoyError::FilesystemError(format!("File '{}' not found", name))
        })?;

        self.read_file_from_location(&location)
    }

    /// Read a file from a specific path
    pub fn read_file_path(&mut self, path: &str) -> Result<Vec<u8>> {
        let resolved = self.resolve_path(path, false)?;

        match resolved.location {
            Some(loc) if loc.is_directory => Err(VentoyError::FilesystemError(format!(
                "'{}' is a directory",
                path
            ))),
            Some(loc) => self.read_file_from_location(&loc),
            None => Err(VentoyError::FilesystemError(format!(
                "File '{}' not found",
                path
            ))),
        }
    }

    /// Get file information at a specific path without reading content
    ///
    /// Returns None if the file doesn't exist.
    pub fn get_file_info_path(&mut self, path: &str) -> Result<Option<FileInfo>> {
        let resolved = self.resolve_path(path, false)?;

        match resolved.location {
            Some(loc) => {
                // Extract just the filename from the path
                let name = path.rsplit('/').next().unwrap_or(path).to_string();
                Ok(Some(FileInfo {
                    name,
                    size: loc.data_length,
                    is_directory: loc.is_directory,
                    path: path.to_string(),
                }))
            }
            None => Ok(None),
        }
    }

    /// Delete a file from the filesystem (root directory)
    pub fn delete_file(&mut self, name: &str) -> Result<()> {
        let location = self.find_file_entry(name)?.ok_or_else(|| {
            VentoyError::FilesystemError(format!("File '{}' not found", name))
        })?;

        // Free cluster chain
        if location.first_cluster >= 2 {
            self.free_cluster_chain(location.first_cluster)?;
        }

        // Delete directory entry
        self.delete_file_entry(&location)?;

        self.file.flush()?;
        Ok(())
    }

    /// Delete a file or directory at a specific path
    pub fn delete_path(&mut self, path: &str) -> Result<()> {
        let resolved = self.resolve_path(path, false)?;

        let location = resolved.location.ok_or_else(|| {
            VentoyError::FilesystemError(format!("'{}' not found", path))
        })?;

        // If it's a directory, check if it's empty
        if location.is_directory {
            let contents = self.list_files_in_directory(location.first_cluster, "")?;
            if !contents.is_empty() {
                return Err(VentoyError::FilesystemError(format!(
                    "Directory '{}' is not empty",
                    path
                )));
            }
        }

        // Free cluster chain
        if location.first_cluster >= 2 {
            self.free_cluster_chain(location.first_cluster)?;
        }

        // Delete directory entry
        self.delete_file_entry(&location)?;

        self.file.flush()?;
        Ok(())
    }

    /// Delete a directory and all its contents recursively
    pub fn delete_recursive(&mut self, path: &str) -> Result<()> {
        let resolved = self.resolve_path(path, false)?;

        let location = resolved.location.ok_or_else(|| {
            VentoyError::FilesystemError(format!("'{}' not found", path))
        })?;

        if location.is_directory {
            // Get all contents and delete them first
            let contents = self.list_files_in_directory(location.first_cluster, "")?;
            for item in contents {
                let item_path = if path.ends_with('/') {
                    format!("{}{}", path, item.name)
                } else {
                    format!("{}/{}", path, item.name)
                };
                self.delete_recursive(&item_path)?;
            }
        }

        // Now delete the item itself
        if location.first_cluster >= 2 {
            self.free_cluster_chain(location.first_cluster)?;
        }
        self.delete_file_entry(&location)?;

        self.file.flush()?;
        Ok(())
    }

    /// Create a directory at a specific path
    ///
    /// If create_parents is true, creates all intermediate directories (mkdir -p behavior)
    pub fn create_directory(&mut self, path: &str, create_parents: bool) -> Result<()> {
        let resolved = self.resolve_path(path, create_parents)?;

        if resolved.location.is_some() {
            return Err(VentoyError::FilesystemError(format!(
                "'{}' already exists",
                path
            )));
        }

        self.create_directory_in(resolved.parent_cluster, &resolved.name)?;
        Ok(())
    }
}

/// Streaming file writer for large files
///
/// This allows writing large files without loading them entirely into memory.
pub struct ExfatFileWriter<'a> {
    fs: &'a mut ExfatFs,
    name: String,
    dir_cluster: u32,
    total_size: u64,
    allocated_clusters: Vec<u32>,
    current_cluster_index: usize,
    cluster_buffer: Vec<u8>,
    bytes_written: u64,
}

impl<'a> ExfatFileWriter<'a> {
    /// Create a new file writer (writes to root directory)
    ///
    /// The total_size must be known in advance to allocate clusters.
    pub fn create(fs: &'a mut ExfatFs, name: &str, total_size: u64) -> Result<Self> {
        let root_cluster = fs.first_cluster_of_root;
        Self::create_in_directory(fs, root_cluster, name, total_size, false)
    }

    /// Create a new file writer with overwrite option
    pub fn create_overwrite(fs: &'a mut ExfatFs, name: &str, total_size: u64, overwrite: bool) -> Result<Self> {
        let root_cluster = fs.first_cluster_of_root;
        Self::create_in_directory(fs, root_cluster, name, total_size, overwrite)
    }

    /// Create a file writer for a specific path
    ///
    /// If create_parents is true, intermediate directories will be created.
    /// If overwrite is true, existing files will be replaced.
    pub fn create_at_path(fs: &'a mut ExfatFs, path: &str, total_size: u64, create_parents: bool, overwrite: bool) -> Result<Self> {
        let resolved = fs.resolve_path(path, create_parents)?;

        // Handle existing file
        if let Some(location) = resolved.location {
            if location.is_directory {
                return Err(VentoyError::FilesystemError(format!(
                    "'{}' is a directory",
                    path
                )));
            }
            if overwrite {
                // Delete existing file
                if location.first_cluster >= 2 {
                    fs.free_cluster_chain(location.first_cluster)?;
                }
                fs.delete_file_entry(&location)?;
            } else {
                return Err(VentoyError::FilesystemError(format!(
                    "File '{}' already exists",
                    path
                )));
            }
        }

        Self::create_in_directory(fs, resolved.parent_cluster, &resolved.name, total_size, false)
    }

    /// Internal: Create a file writer in a specific directory
    fn create_in_directory(fs: &'a mut ExfatFs, dir_cluster: u32, name: &str, total_size: u64, overwrite: bool) -> Result<Self> {
        // Validate filename
        if name.is_empty() || name.len() > 255 {
            return Err(VentoyError::FilesystemError(
                "Invalid filename length".to_string(),
            ));
        }

        // Check if file already exists
        if let Some(location) = fs.find_entry_in_directory(dir_cluster, name)? {
            if overwrite {
                // Delete existing file
                if location.first_cluster >= 2 {
                    fs.free_cluster_chain(location.first_cluster)?;
                }
                fs.delete_file_entry(&location)?;
            } else {
                return Err(VentoyError::FilesystemError(format!(
                    "File '{}' already exists",
                    name
                )));
            }
        }

        // Calculate and allocate clusters
        let clusters_needed = if total_size == 0 {
            0
        } else {
            ((total_size + fs.cluster_size as u64 - 1) / fs.cluster_size as u64) as usize
        };

        let allocated_clusters = if clusters_needed > 0 {
            let first = fs.allocate_clusters(clusters_needed)?;
            fs.read_cluster_chain(first)?
        } else {
            Vec::new()
        };

        let cluster_size = fs.cluster_size as usize;

        Ok(Self {
            fs,
            name: name.to_string(),
            dir_cluster,
            total_size,
            allocated_clusters,
            current_cluster_index: 0,
            cluster_buffer: Vec::with_capacity(cluster_size),
            bytes_written: 0,
        })
    }

    /// Write data to the file
    ///
    /// Returns the number of bytes written.
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        let cluster_size = self.fs.cluster_size as usize;
        let mut data_offset = 0;

        while data_offset < data.len() && self.bytes_written < self.total_size {
            // Fill cluster buffer
            let space_in_buffer = cluster_size - self.cluster_buffer.len();
            let remaining_to_write = (self.total_size - self.bytes_written) as usize;
            let chunk_size = space_in_buffer
                .min(data.len() - data_offset)
                .min(remaining_to_write);

            self.cluster_buffer
                .extend_from_slice(&data[data_offset..data_offset + chunk_size]);
            data_offset += chunk_size;
            self.bytes_written += chunk_size as u64;

            // Write cluster if buffer is full
            if self.cluster_buffer.len() >= cluster_size {
                if self.current_cluster_index < self.allocated_clusters.len() {
                    let cluster = self.allocated_clusters[self.current_cluster_index];
                    self.fs.write_cluster(cluster, &self.cluster_buffer)?;
                    self.current_cluster_index += 1;
                    self.cluster_buffer.clear();
                }
            }
        }

        Ok(data_offset)
    }

    /// Finish writing and create the directory entry
    ///
    /// This must be called after all data has been written.
    pub fn finish(self) -> Result<()> {
        // Write any remaining data in buffer
        if !self.cluster_buffer.is_empty() && self.current_cluster_index < self.allocated_clusters.len() {
            let cluster = self.allocated_clusters[self.current_cluster_index];
            self.fs.write_cluster(cluster, &self.cluster_buffer)?;
        }

        // Create directory entry
        let first_cluster = if self.allocated_clusters.is_empty() {
            0
        } else {
            self.allocated_clusters[0]
        };

        self.fs.create_entry_in_directory(self.dir_cluster, &self.name, first_cluster, self.total_size, false)?;
        self.fs.file.flush()?;

        Ok(())
    }

    /// Get the number of bytes written so far
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

/// Streaming file reader for large files
///
/// This allows reading large files without loading them entirely into memory.
/// Implements `std::io::Read` and `std::io::Seek` traits for compatibility
/// with standard I/O operations.
pub struct ExfatFileReader<'a> {
    fs: &'a mut ExfatFs,
    /// Cluster chain for this file
    cluster_chain: Vec<u32>,
    /// Total file size in bytes
    file_size: u64,
    /// Current position in the file (byte offset from start)
    position: u64,
    /// Cached current cluster data
    cluster_cache: Option<(u32, Vec<u8>)>,
}

impl<'a> ExfatFileReader<'a> {
    /// Open a file for reading from root directory
    pub fn open(fs: &'a mut ExfatFs, name: &str) -> Result<Self> {
        let location = fs.find_file_entry(name)?.ok_or_else(|| {
            VentoyError::FilesystemError(format!("File '{}' not found", name))
        })?;

        if location.is_directory {
            return Err(VentoyError::FilesystemError(format!(
                "'{}' is a directory",
                name
            )));
        }

        Self::from_location(fs, &location)
    }

    /// Open a file for reading from a specific path
    pub fn open_path(fs: &'a mut ExfatFs, path: &str) -> Result<Self> {
        let resolved = fs.resolve_path(path, false)?;

        match resolved.location {
            Some(loc) if loc.is_directory => Err(VentoyError::FilesystemError(format!(
                "'{}' is a directory",
                path
            ))),
            Some(loc) => Self::from_location(fs, &loc),
            None => Err(VentoyError::FilesystemError(format!(
                "File '{}' not found",
                path
            ))),
        }
    }

    /// Internal: Create reader from file entry location
    fn from_location(fs: &'a mut ExfatFs, location: &FileEntryLocation) -> Result<Self> {
        let cluster_chain = if location.first_cluster >= 2 && location.data_length > 0 {
            fs.read_cluster_chain(location.first_cluster)?
        } else {
            Vec::new()
        };

        Ok(Self {
            fs,
            cluster_chain,
            file_size: location.data_length,
            position: 0,
            cluster_cache: None,
        })
    }

    /// Get the total file size
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Get the current position in the file
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get the remaining bytes to read
    pub fn remaining(&self) -> u64 {
        self.file_size.saturating_sub(self.position)
    }

    /// Get cluster size for this filesystem
    fn cluster_size(&self) -> u64 {
        self.fs.cluster_size as u64
    }

    /// Read the cluster at the given index, using cache if available
    fn read_cluster_cached(&mut self, cluster_index: usize) -> Result<&[u8]> {
        if cluster_index >= self.cluster_chain.len() {
            return Err(VentoyError::FilesystemError(
                "Cluster index out of range".to_string(),
            ));
        }

        let cluster_num = self.cluster_chain[cluster_index];

        // Check if we have this cluster cached
        if let Some((cached_cluster, ref data)) = self.cluster_cache {
            if cached_cluster == cluster_num {
                return Ok(unsafe { &*(data.as_slice() as *const [u8]) });
            }
        }

        // Read the cluster
        let data = self.fs.read_cluster(cluster_num)?;
        self.cluster_cache = Some((cluster_num, data));

        Ok(self.cluster_cache.as_ref().unwrap().1.as_slice())
    }
}

impl<'a> Read for ExfatFileReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.position >= self.file_size || buf.is_empty() {
            return Ok(0);
        }

        let cluster_size = self.cluster_size();
        let file_size = self.file_size;
        let mut bytes_read = 0;

        while bytes_read < buf.len() && self.position < file_size {
            // Calculate which cluster we're in and the offset within it
            let cluster_index = (self.position / cluster_size) as usize;
            let offset_in_cluster = (self.position % cluster_size) as usize;

            // Calculate how much we can read from this cluster
            let remaining_in_cluster = cluster_size as usize - offset_in_cluster;
            let remaining_in_file = (file_size - self.position) as usize;
            let remaining_in_buf = buf.len() - bytes_read;
            let to_read = remaining_in_cluster
                .min(remaining_in_file)
                .min(remaining_in_buf);

            // Read the cluster and copy data
            {
                let cluster_data = self.read_cluster_cached(cluster_index)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                // Copy data to buffer
                buf[bytes_read..bytes_read + to_read]
                    .copy_from_slice(&cluster_data[offset_in_cluster..offset_in_cluster + to_read]);
            }

            bytes_read += to_read;
            self.position += to_read as u64;
        }

        Ok(bytes_read)
    }
}

impl<'a> Seek for ExfatFileReader<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.file_size as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };

        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek to negative position",
            ));
        }

        // Allow seeking past end of file (like regular files)
        self.position = new_pos as u64;
        Ok(self.position)
    }
}

impl ExfatFs {
    /// Read a file to a writer (streaming)
    ///
    /// This is useful for reading large files without loading them into memory.
    pub fn read_file_to_writer<W: Write>(
        &mut self,
        name: &str,
        writer: &mut W,
    ) -> Result<u64> {
        let mut reader = ExfatFileReader::open(self, name)?;
        Self::do_stream_read(&mut reader, writer)
    }

    /// Read a file from a path to a writer (streaming)
    pub fn read_file_path_to_writer<W: Write>(
        &mut self,
        path: &str,
        writer: &mut W,
    ) -> Result<u64> {
        let mut reader = ExfatFileReader::open_path(self, path)?;
        Self::do_stream_read(&mut reader, writer)
    }

    /// Internal: Stream read from reader to writer
    fn do_stream_read<W: Write>(reader: &mut ExfatFileReader, writer: &mut W) -> Result<u64> {
        let mut buffer = vec![0u8; 64 * 1024]; // 64KB buffer
        let mut total_bytes = 0u64;

        loop {
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                VentoyError::Io(e)
            })?;
            if bytes_read == 0 {
                break;
            }
            writer.write_all(&buffer[..bytes_read]).map_err(VentoyError::Io)?;
            total_bytes += bytes_read as u64;
        }

        Ok(total_bytes)
    }
}

impl ExfatFs {
    /// Write a file from a reader (streaming)
    ///
    /// This is useful for writing large files without loading them into memory.
    pub fn write_file_from_reader<R: Read>(
        &mut self,
        name: &str,
        reader: &mut R,
        size: u64,
    ) -> Result<()> {
        let mut writer = ExfatFileWriter::create(self, name, size)?;
        Self::do_stream_write(&mut writer, reader)?;
        writer.finish()
    }

    /// Write a file from a reader with overwrite option
    pub fn write_file_from_reader_overwrite<R: Read>(
        &mut self,
        name: &str,
        reader: &mut R,
        size: u64,
        overwrite: bool,
    ) -> Result<()> {
        let mut writer = ExfatFileWriter::create_overwrite(self, name, size, overwrite)?;
        Self::do_stream_write(&mut writer, reader)?;
        writer.finish()
    }

    /// Write a file from a reader to a specific path
    ///
    /// If create_parents is true, intermediate directories will be created.
    /// If overwrite is true, existing files will be replaced.
    pub fn write_file_from_reader_path<R: Read>(
        &mut self,
        path: &str,
        reader: &mut R,
        size: u64,
        create_parents: bool,
        overwrite: bool,
    ) -> Result<()> {
        let mut writer = ExfatFileWriter::create_at_path(self, path, size, create_parents, overwrite)?;
        Self::do_stream_write(&mut writer, reader)?;
        writer.finish()
    }

    /// Internal: Stream write from reader to writer
    fn do_stream_write<R: Read>(writer: &mut ExfatFileWriter, reader: &mut R) -> Result<()> {
        let mut buffer = vec![0u8; 64 * 1024]; // 64KB buffer

        loop {
            let bytes_read = reader.read(&mut buffer).map_err(VentoyError::Io)?;
            if bytes_read == 0 {
                break;
            }
            writer.write(&buffer[..bytes_read])?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partition::PartitionLayout;
    use std::io::Cursor;
    use tempfile::NamedTempFile;

    /// Test directory extension when filling up a directory cluster
    #[test]
    fn test_directory_extension() -> Result<()> {
        // Create a small test image (64MB minimum) with small cluster size (4KB)
        // This makes it easier to fill up a directory cluster
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create 64MB image (minimum size)
        let size = 64 * 1024 * 1024u64;
        let layout = PartitionLayout::calculate(size).unwrap();

        // Initialize file
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        file.set_len(size).unwrap();

        // Format data partition (this will use 4KB clusters for 64MB volume)
        crate::exfat::format::format_exfat(&mut file, layout.data_offset(), layout.data_size(), "TEST")
            .unwrap();

        drop(file);

        // Open filesystem
        let mut fs = ExfatFs::open(path, &layout).unwrap();

        // Calculate how many files fit in one 4KB cluster
        // Each file needs: 1 file entry (32 bytes) + 1 stream entry (32 bytes) + name entries
        // For short names (e.g., "f1.txt"), we need 1 name entry (32 bytes)
        // Total: 3 * 32 = 96 bytes per file
        // 4KB = 4096 bytes, so ~42 files per cluster
        // But root directory already has volume label, bitmap, upcase entries
        // Let's add 50 small files to ensure we exceed one cluster

        // Add many small files to fill up the first directory cluster
        for i in 0..50 {
            let filename = format!("file{}.txt", i);
            let data = format!("content {}", i);
            let mut cursor = Cursor::new(data.as_bytes());

            fs.write_file_from_reader(
                &filename,
                &mut cursor,
                data.len() as u64,
            )?;
        }

        // Verify all files were created
        let files = fs.list_files().unwrap();
        assert_eq!(
            files.len(),
            50,
            "Expected 50 files, found {}",
            files.len()
        );

        // Verify we can read all files back
        for i in 0..50 {
            let filename = format!("file{}.txt", i);
            let expected = format!("content {}", i);
            let data = fs.read_file(&filename).unwrap();
            let content = String::from_utf8(data).unwrap();
            assert_eq!(content, expected, "File {} content mismatch", filename);
        }

        // Verify directory chain was extended
        // Root directory starts at one cluster, should now have multiple clusters
        let root_chain = fs.read_cluster_chain(fs.first_cluster_of_root).unwrap();
        assert!(
            root_chain.len() > 1,
            "Expected directory to extend beyond 1 cluster, got {} clusters",
            root_chain.len()
        );

        Ok(())
    }

    /// Test streaming file reader
    #[test]
    fn test_streaming_read() -> Result<()> {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create 64MB image
        let size = 64 * 1024 * 1024u64;
        let layout = PartitionLayout::calculate(size).unwrap();

        // Initialize file
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        file.set_len(size).unwrap();

        // Format data partition
        crate::exfat::format::format_exfat(&mut file, layout.data_offset(), layout.data_size(), "TEST")
            .unwrap();

        drop(file);

        // Open filesystem and write test file
        let mut fs = ExfatFs::open(path, &layout).unwrap();

        // Create test data spanning multiple clusters (4KB cluster size for 64MB)
        // Create 20KB of test data (5 clusters)
        let test_data: Vec<u8> = (0..20480).map(|i| (i % 256) as u8).collect();
        let mut cursor = Cursor::new(&test_data);
        fs.write_file_from_reader("large_file.bin", &mut cursor, test_data.len() as u64)?;

        // Test 1: Stream read entire file using ExfatFileReader
        {
            let mut reader = ExfatFileReader::open(&mut fs, "large_file.bin")?;
            assert_eq!(reader.file_size(), test_data.len() as u64);
            assert_eq!(reader.position(), 0);

            let mut read_data = Vec::new();
            let bytes_read = reader.read_to_end(&mut read_data)
                .map_err(|e| VentoyError::Io(e))?;

            assert_eq!(bytes_read, test_data.len());
            assert_eq!(read_data, test_data);
        }

        // Test 2: Stream read with small buffer (simulating streaming)
        {
            let mut reader = ExfatFileReader::open(&mut fs, "large_file.bin")?;
            let mut read_data = Vec::new();
            let mut buffer = [0u8; 1024]; // 1KB buffer

            loop {
                let n = reader.read(&mut buffer).map_err(|e| VentoyError::Io(e))?;
                if n == 0 {
                    break;
                }
                read_data.extend_from_slice(&buffer[..n]);
            }

            assert_eq!(read_data, test_data);
        }

        // Test 3: Seek operations
        {
            let mut reader = ExfatFileReader::open(&mut fs, "large_file.bin")?;

            // Seek to middle
            reader.seek(SeekFrom::Start(10000)).map_err(|e| VentoyError::Io(e))?;
            assert_eq!(reader.position(), 10000);

            let mut buffer = [0u8; 10];
            reader.read_exact(&mut buffer).map_err(|e| VentoyError::Io(e))?;
            assert_eq!(&buffer, &test_data[10000..10010]);

            // Seek from current position
            reader.seek(SeekFrom::Current(-5)).map_err(|e| VentoyError::Io(e))?;
            assert_eq!(reader.position(), 10005);

            // Seek from end
            reader.seek(SeekFrom::End(-100)).map_err(|e| VentoyError::Io(e))?;
            assert_eq!(reader.position(), test_data.len() as u64 - 100);

            reader.read_exact(&mut buffer).map_err(|e| VentoyError::Io(e))?;
            let expected_start = test_data.len() - 100;
            assert_eq!(&buffer, &test_data[expected_start..expected_start + 10]);
        }

        // Test 4: read_file_to_writer streaming API
        {
            let mut output = Vec::new();
            fs.read_file_to_writer("large_file.bin", &mut output)?;
            assert_eq!(output, test_data);
        }

        Ok(())
    }

    /// Test Unicode file names (CJK, Cyrillic, emoji, etc.)
    #[test]
    fn test_unicode_filenames() -> Result<()> {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create 64MB image
        let size = 64 * 1024 * 1024u64;
        let layout = PartitionLayout::calculate(size).unwrap();

        // Initialize file
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        file.set_len(size).unwrap();

        // Format data partition
        crate::exfat::format::format_exfat(&mut file, layout.data_offset(), layout.data_size(), "TEST")
            .unwrap();

        drop(file);

        // Open filesystem
        let mut fs = ExfatFs::open(path, &layout).unwrap();

        // Test 1: CJK characters (Chinese, Japanese, Korean)
        let cjk_content = b"Hello from CJK file";
        fs.write_file("中文文件.txt", cjk_content)?;
        fs.write_file("日本語ファイル.txt", cjk_content)?;
        fs.write_file("한국어파일.txt", cjk_content)?;

        // Verify CJK files exist and can be read
        let read_data = fs.read_file("中文文件.txt")?;
        assert_eq!(read_data, cjk_content);

        let read_data = fs.read_file("日本語ファイル.txt")?;
        assert_eq!(read_data, cjk_content);

        let read_data = fs.read_file("한국어파일.txt")?;
        assert_eq!(read_data, cjk_content);

        // Test 2: Cyrillic characters (Russian)
        let cyrillic_content = b"Cyrillic content";
        fs.write_file("Русский файл.txt", cyrillic_content)?;
        let read_data = fs.read_file("Русский файл.txt")?;
        assert_eq!(read_data, cyrillic_content);

        // Test 3: Latin Extended (accented characters)
        let latin_content = b"Latin extended content";
        fs.write_file("Ñoño_Résumé_Naïve.txt", latin_content)?;
        let read_data = fs.read_file("Ñoño_Résumé_Naïve.txt")?;
        assert_eq!(read_data, latin_content);

        // Test 4: Greek characters
        let greek_content = b"Greek content";
        fs.write_file("Ελληνικά.txt", greek_content)?;
        let read_data = fs.read_file("Ελληνικά.txt")?;
        assert_eq!(read_data, greek_content);

        // Test 5: Emoji (surrogate pairs in UTF-16)
        let emoji_content = b"Emoji content";
        fs.write_file("😀🎉🚀.txt", emoji_content)?;
        let read_data = fs.read_file("😀🎉🚀.txt")?;
        assert_eq!(read_data, emoji_content);

        // Test 6: Mixed script file name
        let mixed_content = b"Mixed content";
        fs.write_file("Hello世界Привет🌍.txt", mixed_content)?;
        let read_data = fs.read_file("Hello世界Привет🌍.txt")?;
        assert_eq!(read_data, mixed_content);

        // Test 7: List all files and verify Unicode names are preserved
        let files = fs.list_files()?;
        let file_names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();

        assert!(file_names.contains(&"中文文件.txt"));
        assert!(file_names.contains(&"日本語ファイル.txt"));
        assert!(file_names.contains(&"한국어파일.txt"));
        assert!(file_names.contains(&"Русский файл.txt"));
        assert!(file_names.contains(&"Ñoño_Résumé_Naïve.txt"));
        assert!(file_names.contains(&"Ελληνικά.txt"));
        assert!(file_names.contains(&"😀🎉🚀.txt"));
        assert!(file_names.contains(&"Hello世界Привет🌍.txt"));

        // Test 8: Delete Unicode file
        fs.delete_file("😀🎉🚀.txt")?;
        let files = fs.list_files()?;
        let file_names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert!(!file_names.contains(&"😀🎉🚀.txt"));

        Ok(())
    }
}
