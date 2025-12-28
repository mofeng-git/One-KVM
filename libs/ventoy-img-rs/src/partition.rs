//! MBR partition table implementation

use crate::error::{Result, VentoyError};
use std::io::{Seek, SeekFrom, Write};

/// Sector size in bytes
pub const SECTOR_SIZE: u64 = 512;

/// Data partition starts at sector 2048 (1MB aligned)
pub const DATA_PART_START_SECTOR: u64 = 2048;

/// EFI partition size: 32MB = 65536 sectors
pub const EFI_PART_SIZE_SECTORS: u64 = 65536;

/// Minimum image size: 64MB
pub const MIN_IMAGE_SIZE: u64 = 64 * 1024 * 1024;

/// MBR partition type: NTFS/exFAT (0x07)
pub const MBR_TYPE_EXFAT: u8 = 0x07;

/// MBR partition type: EFI System (0xEF)
pub const MBR_TYPE_EFI: u8 = 0xEF;

/// Ventoy signature offset in MBR
pub const VENTOY_SIG_OFFSET: u64 = 0x190; // 400

/// Partition layout information
#[derive(Debug, Clone)]
pub struct PartitionLayout {
    pub total_sectors: u64,
    pub data_start_sector: u64,
    pub data_size_sectors: u64,
    pub efi_start_sector: u64,
    pub efi_size_sectors: u64,
}

impl PartitionLayout {
    /// Calculate partition layout for given image size
    pub fn calculate(total_size: u64) -> Result<Self> {
        if total_size < MIN_IMAGE_SIZE {
            return Err(VentoyError::InvalidSize(format!(
                "{}MB (minimum 64MB)",
                total_size / (1024 * 1024)
            )));
        }

        let total_sectors = total_size / SECTOR_SIZE;

        // EFI partition at the end, 4KB aligned
        let efi_start = ((total_sectors - EFI_PART_SIZE_SECTORS) / 8) * 8;

        // Data partition fills the gap
        let data_size = efi_start - DATA_PART_START_SECTOR;

        Ok(Self {
            total_sectors,
            data_start_sector: DATA_PART_START_SECTOR,
            data_size_sectors: data_size,
            efi_start_sector: efi_start,
            efi_size_sectors: EFI_PART_SIZE_SECTORS,
        })
    }

    /// Get data partition offset in bytes
    pub fn data_offset(&self) -> u64 {
        self.data_start_sector * SECTOR_SIZE
    }

    /// Get data partition size in bytes
    pub fn data_size(&self) -> u64 {
        self.data_size_sectors * SECTOR_SIZE
    }

    /// Get EFI partition offset in bytes
    pub fn efi_offset(&self) -> u64 {
        self.efi_start_sector * SECTOR_SIZE
    }
}

/// MBR partition entry (16 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct MbrPartitionEntry {
    boot_indicator: u8,
    start_chs: [u8; 3],
    partition_type: u8,
    end_chs: [u8; 3],
    start_lba: u32,
    size_sectors: u32,
}

impl MbrPartitionEntry {
    fn new(bootable: bool, partition_type: u8, start_lba: u64, size_sectors: u64) -> Self {
        Self {
            boot_indicator: if bootable { 0x80 } else { 0x00 },
            start_chs: [0xFE, 0xFF, 0xFF], // LBA mode
            partition_type,
            end_chs: [0xFE, 0xFF, 0xFF], // LBA mode
            start_lba: start_lba as u32,
            size_sectors: size_sectors as u32,
        }
    }

    fn to_bytes(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0] = self.boot_indicator;
        bytes[1..4].copy_from_slice(&self.start_chs);
        bytes[4] = self.partition_type;
        bytes[5..8].copy_from_slice(&self.end_chs);
        bytes[8..12].copy_from_slice(&self.start_lba.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.size_sectors.to_le_bytes());
        bytes
    }
}

/// Write MBR partition table to image
pub fn write_mbr_partition_table<W: Write + Seek>(
    writer: &mut W,
    layout: &PartitionLayout,
) -> Result<()> {
    // Partition 1: Data partition (exFAT, bootable)
    let part1 = MbrPartitionEntry::new(
        true,
        MBR_TYPE_EXFAT,
        layout.data_start_sector,
        layout.data_size_sectors,
    );

    // Partition 2: EFI System partition
    let part2 = MbrPartitionEntry::new(
        false,
        MBR_TYPE_EFI,
        layout.efi_start_sector,
        layout.efi_size_sectors,
    );

    // Write partition table entries (offset 0x1BE = 446)
    writer.seek(SeekFrom::Start(446))?;
    writer.write_all(&part1.to_bytes())?;
    writer.write_all(&part2.to_bytes())?;

    // Clear partition 3 and 4
    writer.write_all(&[0u8; 32])?;

    // Write MBR signature (0x55AA)
    writer.seek(SeekFrom::Start(510))?;
    writer.write_all(&[0x55, 0xAA])?;

    Ok(())
}

/// Parse size string like "8G", "1024M" into bytes
pub fn parse_size(s: &str) -> Result<u64> {
    let s = s.trim().to_uppercase();

    let (num_str, multiplier) = if s.ends_with('G') {
        (&s[..s.len() - 1], 1024 * 1024 * 1024u64)
    } else if s.ends_with('M') {
        (&s[..s.len() - 1], 1024 * 1024u64)
    } else if s.ends_with('K') {
        (&s[..s.len() - 1], 1024u64)
    } else {
        (s.as_str(), 1u64)
    };

    let num: u64 = num_str
        .parse()
        .map_err(|_| VentoyError::SizeParseError(s.clone()))?;

    Ok(num * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("8G").unwrap(), 8 * 1024 * 1024 * 1024);
        assert_eq!(parse_size("1024M").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("512K").unwrap(), 512 * 1024);
    }

    #[test]
    fn test_partition_layout() {
        let layout = PartitionLayout::calculate(8 * 1024 * 1024 * 1024).unwrap();
        assert_eq!(layout.data_start_sector, 2048);
        assert_eq!(layout.efi_size_sectors, 65536);
        assert!(layout.efi_start_sector > layout.data_start_sector);
    }
}
