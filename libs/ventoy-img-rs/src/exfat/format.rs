//! exFAT filesystem formatting

use crate::error::Result;
use crate::exfat::unicode;
use std::io::{Seek, SeekFrom, Write};

/// exFAT cluster size based on volume size
///
/// exFAT specification recommendations:
/// - < 256MB: 4KB clusters
/// - 256MB - 32GB: 32KB clusters
/// - 32GB - 256GB: 128KB clusters
/// - > 256GB: 256KB clusters (but we cap at 128KB for simplicity)
///
/// Note: Smaller clusters reduce waste but increase FAT table size and metadata overhead.
/// Larger clusters improve performance but waste space on small files.
fn get_cluster_size(total_sectors: u64) -> u32 {
    let volume_size = total_sectors * 512; // Convert to bytes

    match volume_size {
        // < 256MB: Use 4KB clusters (good for many small files)
        n if n < 256 * 1024 * 1024 => 4096,
        // 256MB - 8GB: Use 32KB clusters (balanced)
        n if n < 8 * 1024 * 1024 * 1024 => 32768,
        // 8GB - 256GB: Use 128KB clusters (optimal for large ISOs)
        _ => 128 * 1024,
    }
}

/// Calculate sectors per cluster shift
///
/// Returns the power of 2 for sectors per cluster (512-byte sectors).
/// For example: 32KB cluster = 64 sectors = 2^6, so shift = 6
fn sectors_per_cluster_shift(cluster_size: u32) -> u8 {
    match cluster_size {
        4096 => 3,   // 8 sectors (4KB)
        8192 => 4,   // 16 sectors (8KB)
        16384 => 5,  // 32 sectors (16KB)
        32768 => 6,  // 64 sectors (32KB)
        65536 => 7,  // 128 sectors (64KB)
        131072 => 8, // 256 sectors (128KB)
        262144 => 9, // 512 sectors (256KB)
        _ => {
            // Fallback: calculate dynamically
            let sectors = cluster_size / 512;
            (sectors.trailing_zeros() as u8).max(3).min(9)
        }
    }
}

/// exFAT Boot Sector (512 bytes)
#[repr(C, packed)]
struct ExfatBootSector {
    jump_boot: [u8; 3],
    fs_name: [u8; 8],
    must_be_zero: [u8; 53],
    partition_offset: u64,
    volume_length: u64,
    fat_offset: u32,
    fat_length: u32,
    cluster_heap_offset: u32,
    cluster_count: u32,
    first_cluster_of_root: u32,
    volume_serial_number: u32,
    fs_revision: u16,
    volume_flags: u16,
    bytes_per_sector_shift: u8,
    sectors_per_cluster_shift: u8,
    number_of_fats: u8,
    drive_select: u8,
    percent_in_use: u8,
    reserved: [u8; 7],
    boot_code: [u8; 390],
    boot_signature: u16,
}

impl ExfatBootSector {
    fn new(volume_length: u64, cluster_size: u32, volume_serial: u32) -> Self {
        let sector_size: u32 = 512;
        let sectors_per_cluster = cluster_size / sector_size;
        let spc_shift = sectors_per_cluster_shift(cluster_size);

        // Calculate FAT offset (after boot region, typically sector 24)
        let fat_offset: u32 = 24;

        // Calculate cluster count and FAT length
        // Cluster heap starts after FAT region
        let usable_sectors = volume_length as u32 - fat_offset;
        let cluster_count = (usable_sectors - 32) / sectors_per_cluster; // rough estimate
        let fat_entries = cluster_count + 2; // cluster 0 and 1 are reserved
        let fat_length = ((fat_entries * 4 + sector_size - 1) / sector_size).max(1);

        // Cluster heap offset
        let cluster_heap_offset = fat_offset + fat_length;

        // Recalculate cluster count
        let heap_sectors = volume_length as u32 - cluster_heap_offset;
        let cluster_count = heap_sectors / sectors_per_cluster;

        // Calculate root directory cluster based on bitmap and upcase table size.
        const UPCASE_TABLE_SIZE: u64 = 128 * 1024;
        let bitmap_size = ((cluster_count + 7) / 8) as u64;
        let bitmap_clusters =
            ((bitmap_size + cluster_size as u64 - 1) / cluster_size as u64).max(1) as u32;
        let upcase_clusters =
            ((UPCASE_TABLE_SIZE + cluster_size as u64 - 1) / cluster_size as u64) as u32;
        let first_cluster_of_root = 2 + bitmap_clusters + upcase_clusters;

        Self {
            jump_boot: [0xEB, 0x76, 0x90],
            fs_name: *b"EXFAT   ",
            must_be_zero: [0; 53],
            partition_offset: 0,
            volume_length,
            fat_offset,
            fat_length,
            cluster_heap_offset,
            cluster_count,
            first_cluster_of_root,
            volume_serial_number: volume_serial,
            fs_revision: 0x0100,
            volume_flags: 0,
            bytes_per_sector_shift: 9, // 512 bytes
            sectors_per_cluster_shift: spc_shift,
            number_of_fats: 1,
            drive_select: 0x80,
            percent_in_use: 0xFF,
            reserved: [0; 7],
            boot_code: [0; 390],
            boot_signature: 0xAA55,
        }
    }

    fn to_bytes(&self) -> [u8; 512] {
        let mut bytes = [0u8; 512];

        bytes[0..3].copy_from_slice(&self.jump_boot);
        bytes[3..11].copy_from_slice(&self.fs_name);
        // bytes[11..64] already zero (must_be_zero)
        bytes[64..72].copy_from_slice(&self.partition_offset.to_le_bytes());
        bytes[72..80].copy_from_slice(&self.volume_length.to_le_bytes());
        bytes[80..84].copy_from_slice(&self.fat_offset.to_le_bytes());
        bytes[84..88].copy_from_slice(&self.fat_length.to_le_bytes());
        bytes[88..92].copy_from_slice(&self.cluster_heap_offset.to_le_bytes());
        bytes[92..96].copy_from_slice(&self.cluster_count.to_le_bytes());
        bytes[96..100].copy_from_slice(&self.first_cluster_of_root.to_le_bytes());
        bytes[100..104].copy_from_slice(&self.volume_serial_number.to_le_bytes());
        bytes[104..106].copy_from_slice(&self.fs_revision.to_le_bytes());
        bytes[106..108].copy_from_slice(&self.volume_flags.to_le_bytes());
        bytes[108] = self.bytes_per_sector_shift;
        bytes[109] = self.sectors_per_cluster_shift;
        bytes[110] = self.number_of_fats;
        bytes[111] = self.drive_select;
        bytes[112] = self.percent_in_use;
        // bytes[113..120] reserved
        // bytes[120..510] boot_code
        bytes[510..512].copy_from_slice(&self.boot_signature.to_le_bytes());

        bytes
    }
}

/// Calculate boot checksum for exFAT
fn calculate_boot_checksum(sectors: &[[u8; 512]; 11]) -> u32 {
    let mut checksum: u32 = 0;

    for (sector_idx, sector) in sectors.iter().enumerate() {
        for (byte_idx, &byte) in sector.iter().enumerate() {
            // Skip VolumeFlags and PercentInUse fields in boot sector
            if sector_idx == 0 && (byte_idx == 106 || byte_idx == 107 || byte_idx == 112) {
                continue;
            }
            checksum = if checksum & 1 != 0 {
                0x80000000 | (checksum >> 1)
            } else {
                checksum >> 1
            };
            checksum = checksum.wrapping_add(byte as u32);
        }
    }

    checksum
}

/// Upcase table with Unicode support
///
/// Uses the unicode module for proper uppercase conversion
/// of international characters (Latin Extended, Greek, Cyrillic, etc.)
fn generate_upcase_table() -> Vec<u8> {
    unicode::generate_upcase_table()
}

/// Calculate upcase table checksum
fn calculate_upcase_checksum(data: &[u8]) -> u32 {
    let mut checksum: u32 = 0;

    for &byte in data {
        checksum = if checksum & 1 != 0 {
            0x80000000 | (checksum >> 1)
        } else {
            checksum >> 1
        };
        checksum = checksum.wrapping_add(byte as u32);
    }

    checksum
}

/// Directory entry types
const ENTRY_TYPE_VOLUME_LABEL: u8 = 0x83;
const ENTRY_TYPE_BITMAP: u8 = 0x81;
const ENTRY_TYPE_UPCASE: u8 = 0x82;

fn set_cluster_allocated(bitmap: &mut [u8], cluster: u32) {
    let index = (cluster - 2) as usize;
    let byte_idx = index / 8;
    let bit_idx = index % 8;
    if byte_idx < bitmap.len() {
        bitmap[byte_idx] |= 1 << bit_idx;
    }
}

/// Create volume label directory entry
fn create_volume_label_entry(label: &str) -> [u8; 32] {
    let mut entry = [0u8; 32];
    entry[0] = ENTRY_TYPE_VOLUME_LABEL;

    let label_chars: Vec<u16> = label.encode_utf16().take(11).collect();
    entry[1] = label_chars.len() as u8;

    for (i, &ch) in label_chars.iter().enumerate() {
        let offset = 2 + i * 2;
        entry[offset..offset + 2].copy_from_slice(&ch.to_le_bytes());
    }

    entry
}

/// Create bitmap directory entry
fn create_bitmap_entry(start_cluster: u32, size: u64) -> [u8; 32] {
    let mut entry = [0u8; 32];
    entry[0] = ENTRY_TYPE_BITMAP;
    entry[1] = 0; // BitmapFlags
                  // Reserved: bytes 2-19
    entry[20..24].copy_from_slice(&start_cluster.to_le_bytes());
    entry[24..32].copy_from_slice(&size.to_le_bytes());
    entry
}

/// Create upcase table directory entry
fn create_upcase_entry(start_cluster: u32, size: u64, checksum: u32) -> [u8; 32] {
    let mut entry = [0u8; 32];
    entry[0] = ENTRY_TYPE_UPCASE;
    // Reserved: bytes 1-3
    entry[4..8].copy_from_slice(&checksum.to_le_bytes());
    // Reserved: bytes 8-19
    entry[20..24].copy_from_slice(&start_cluster.to_le_bytes());
    entry[24..32].copy_from_slice(&size.to_le_bytes());
    entry
}

/// Format a partition as exFAT
pub fn format_exfat<W: Write + Seek>(
    writer: &mut W,
    partition_offset: u64,
    partition_size: u64,
    label: &str,
) -> Result<()> {
    let volume_sectors = partition_size / 512;
    let cluster_size = get_cluster_size(volume_sectors);
    let _sectors_per_cluster = cluster_size / 512;

    // Generate volume serial from timestamp
    let serial = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0x12345678);

    // Create boot sector
    let boot_sector = ExfatBootSector::new(volume_sectors, cluster_size, serial);
    let boot_bytes = boot_sector.to_bytes();

    // Prepare boot region (12 sectors)
    let mut boot_region: [[u8; 512]; 11] = [[0; 512]; 11];
    boot_region[0] = boot_bytes;
    // Sectors 1-8: Extended boot sectors (can be zero)
    // Sector 9-10: OEM parameters (can be zero)

    // Calculate boot checksum
    let checksum = calculate_boot_checksum(&boot_region);
    let mut checksum_sector = [0u8; 512];
    for i in 0..128 {
        checksum_sector[i * 4..(i + 1) * 4].copy_from_slice(&checksum.to_le_bytes());
    }

    // Write main boot region (sectors 0-11)
    writer.seek(SeekFrom::Start(partition_offset))?;
    for sector in &boot_region {
        writer.write_all(sector)?;
    }
    writer.write_all(&checksum_sector)?;

    // Write backup boot region (sectors 12-23)
    for sector in &boot_region {
        writer.write_all(sector)?;
    }
    writer.write_all(&checksum_sector)?;

    // Write FAT
    let fat_offset = partition_offset + boot_sector.fat_offset as u64 * 512;
    writer.seek(SeekFrom::Start(fat_offset))?;

    let bitmap_size = (boot_sector.cluster_count + 7) / 8;
    let bitmap_clusters =
        ((bitmap_size as u64 + cluster_size as u64 - 1) / cluster_size as u64).max(1) as u32;

    // Calculate how many clusters the upcase table needs (128KB)
    const UPCASE_TABLE_SIZE: u64 = 128 * 1024;
    let upcase_clusters =
        ((UPCASE_TABLE_SIZE + cluster_size as u64 - 1) / cluster_size as u64) as u32;
    let bitmap_start_cluster = 2;
    let upcase_start_cluster = bitmap_start_cluster + bitmap_clusters;
    let root_cluster = upcase_start_cluster + upcase_clusters;

    // FAT entries: cluster 0 and 1 are reserved
    // 0: Media type (0xFFFFFFF8)
    // 1: Reserved (0xFFFFFFFF)
    // 2..2+bitmap_clusters-1: Bitmap cluster chain
    // upcase_start_cluster..upcase_start_cluster+upcase_clusters-1: Upcase table cluster chain
    // root_cluster: Root directory cluster (end of chain)
    let mut fat_entries = vec![
        0xFFFFFFF8, // Media type
        0xFFFFFFFF, // Reserved
    ];

    // Build allocation bitmap cluster chain
    for i in 0..bitmap_clusters {
        let cluster_num = bitmap_start_cluster + i;
        if i == bitmap_clusters - 1 {
            fat_entries.push(0xFFFFFFFF);
        } else {
            fat_entries.push(cluster_num + 1);
        }
    }

    // Build upcase table cluster chain
    for i in 0..upcase_clusters {
        let cluster_num = upcase_start_cluster + i;
        if i == upcase_clusters - 1 {
            // Last cluster in chain
            fat_entries.push(0xFFFFFFFF);
        } else {
            // Point to next cluster
            fat_entries.push(cluster_num + 1);
        }
    }

    // Root directory (single cluster, end of chain)
    fat_entries.push(0xFFFFFFFF);

    for entry in &fat_entries {
        writer.write_all(&entry.to_le_bytes())?;
    }

    // Zero fill rest of FAT
    let fat_remaining = (boot_sector.fat_length as usize * 512) - (fat_entries.len() * 4);
    writer.write_all(&vec![0u8; fat_remaining])?;

    // Calculate cluster heap offset
    let heap_offset = partition_offset + boot_sector.cluster_heap_offset as u64 * 512;

    // Allocation Bitmap
    let mut bitmap = vec![0u8; bitmap_clusters as usize * cluster_size as usize];

    // Mark bitmap, upcase, and root directory clusters as used.
    // exFAT allocation bitmap bit 0 describes cluster 2.
    for i in 0..bitmap_clusters {
        set_cluster_allocated(&mut bitmap, bitmap_start_cluster + i);
    }
    for i in 0..upcase_clusters {
        set_cluster_allocated(&mut bitmap, upcase_start_cluster + i);
    }
    set_cluster_allocated(&mut bitmap, root_cluster);

    writer.seek(SeekFrom::Start(heap_offset))?;
    writer.write_all(&bitmap)?;

    // Upcase table
    let upcase_data = generate_upcase_table();
    let upcase_checksum = calculate_upcase_checksum(&upcase_data);
    let upcase_offset = heap_offset + bitmap_clusters as u64 * cluster_size as u64;
    writer.seek(SeekFrom::Start(upcase_offset))?;
    writer.write_all(&upcase_data)?;

    // Pad to fill all upcase clusters
    let upcase_total_size = upcase_clusters as usize * cluster_size as usize;
    let upcase_padding = upcase_total_size - upcase_data.len();
    if upcase_padding > 0 {
        writer.write_all(&vec![0u8; upcase_padding])?;
    }

    // Root directory cluster
    let root_offset =
        heap_offset + (bitmap_clusters as u64 + upcase_clusters as u64) * cluster_size as u64;
    writer.seek(SeekFrom::Start(root_offset))?;

    // Write directory entries
    let volume_label_entry = create_volume_label_entry(label);
    let bitmap_entry = create_bitmap_entry(bitmap_start_cluster, bitmap_size as u64);
    let upcase_entry = create_upcase_entry(
        upcase_start_cluster,
        upcase_data.len() as u64,
        upcase_checksum,
    );

    writer.write_all(&volume_label_entry)?;
    writer.write_all(&bitmap_entry)?;
    writer.write_all(&upcase_entry)?;

    // Pad root directory to cluster size
    let root_used = 32 * 3;
    let root_padding = cluster_size as usize - root_used;
    writer.write_all(&vec![0u8; root_padding])?;

    writer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partition::PartitionLayout;
    use std::io::Read;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cluster_size() {
        // < 256MB: 4KB clusters
        assert_eq!(get_cluster_size(200 * 2048), 4096); // 100MB → 4KB
        assert_eq!(get_cluster_size(100 * 1024 * 1024 / 512), 4096); // 100MB → 4KB

        // 256MB - 8GB: 32KB clusters
        assert_eq!(get_cluster_size(512 * 1024 * 1024 / 512), 32768); // 512MB → 32KB
        assert_eq!(get_cluster_size(4 * 1024 * 1024 * 1024 / 512), 32768); // 4GB → 32KB

        // >= 8GB: 128KB clusters
        assert_eq!(get_cluster_size(8 * 1024 * 1024 * 1024 / 512), 131072); // 8GB → 128KB
        assert_eq!(get_cluster_size(16 * 1024 * 1024 * 1024 / 512), 131072); // 16GB → 128KB
    }

    #[test]
    fn test_format_bitmap_uses_cluster_heap_bit_indices() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let size = 64 * 1024 * 1024u64;
        let layout = PartitionLayout::calculate(size).unwrap();

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        file.set_len(size).unwrap();
        format_exfat(&mut file, layout.data_offset(), layout.data_size(), "TEST").unwrap();

        let mut boot_sector = [0u8; 512];
        file.seek(SeekFrom::Start(layout.data_offset())).unwrap();
        file.read_exact(&mut boot_sector).unwrap();
        let cluster_heap_offset = u32::from_le_bytes(boot_sector[88..92].try_into().unwrap());
        let first_cluster_of_root = u32::from_le_bytes(boot_sector[96..100].try_into().unwrap());
        let sectors_per_cluster = 1u64 << boot_sector[109];
        let cluster_size = sectors_per_cluster * 512;

        let bitmap_offset = layout.data_offset() + cluster_heap_offset as u64 * 512;
        let mut bitmap = vec![0u8; cluster_size as usize];
        file.seek(SeekFrom::Start(bitmap_offset)).unwrap();
        file.read_exact(&mut bitmap).unwrap();

        let is_allocated = |cluster: u32| {
            let index = (cluster - 2) as usize;
            (bitmap[index / 8] & (1 << (index % 8))) != 0
        };

        assert!(
            is_allocated(2),
            "allocation bitmap cluster must be allocated"
        );
        assert!(
            is_allocated(3),
            "upcase table first cluster must be allocated"
        );
        assert!(
            is_allocated(first_cluster_of_root),
            "root directory cluster must be allocated"
        );
        assert!(
            !is_allocated(first_cluster_of_root + 1),
            "first data cluster after root should be free after formatting"
        );
    }

    #[test]
    fn test_format_supports_multi_cluster_allocation_bitmap() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let size = 240 * 1024 * 1024u64;
        let layout = PartitionLayout::calculate(size).unwrap();

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        file.set_len(size).unwrap();
        format_exfat(&mut file, layout.data_offset(), layout.data_size(), "TEST").unwrap();

        let mut boot_sector = [0u8; 512];
        file.seek(SeekFrom::Start(layout.data_offset())).unwrap();
        file.read_exact(&mut boot_sector).unwrap();
        let fat_offset = u32::from_le_bytes(boot_sector[80..84].try_into().unwrap());
        let cluster_heap_offset = u32::from_le_bytes(boot_sector[88..92].try_into().unwrap());
        let cluster_count = u32::from_le_bytes(boot_sector[92..96].try_into().unwrap());
        let first_cluster_of_root = u32::from_le_bytes(boot_sector[96..100].try_into().unwrap());
        let sectors_per_cluster = 1u64 << boot_sector[109];
        let cluster_size = sectors_per_cluster * 512;

        let bitmap_size = ((cluster_count + 7) / 8) as u64;
        let bitmap_clusters = bitmap_size.div_ceil(cluster_size) as u32;
        assert!(
            bitmap_clusters > 1,
            "test volume should require a multi-cluster allocation bitmap"
        );

        let upcase_clusters = (128 * 1024u64).div_ceil(cluster_size) as u32;
        assert_eq!(first_cluster_of_root, 2 + bitmap_clusters + upcase_clusters);

        let read_fat = |file: &mut std::fs::File, cluster: u32| -> u32 {
            let offset = layout.data_offset() + fat_offset as u64 * 512 + cluster as u64 * 4;
            let mut bytes = [0u8; 4];
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.read_exact(&mut bytes).unwrap();
            u32::from_le_bytes(bytes)
        };
        assert_eq!(read_fat(&mut file, 2), 3);
        assert_eq!(read_fat(&mut file, 2 + bitmap_clusters - 1), 0xFFFFFFFF);

        let root_offset = layout.data_offset()
            + cluster_heap_offset as u64 * 512
            + (first_cluster_of_root - 2) as u64 * cluster_size;
        let mut root = vec![0u8; cluster_size as usize];
        file.seek(SeekFrom::Start(root_offset)).unwrap();
        file.read_exact(&mut root).unwrap();

        assert_eq!(root[32], ENTRY_TYPE_BITMAP);
        assert_eq!(u32::from_le_bytes(root[52..56].try_into().unwrap()), 2);
        assert_eq!(
            u64::from_le_bytes(root[56..64].try_into().unwrap()),
            bitmap_size
        );
        assert_eq!(root[64], ENTRY_TYPE_UPCASE);
        assert_eq!(
            u32::from_le_bytes(root[84..88].try_into().unwrap()),
            2 + bitmap_clusters
        );
    }
}
