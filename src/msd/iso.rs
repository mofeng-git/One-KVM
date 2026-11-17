//! Native ISO 9660 / Joliet / UDF probing.
//!
//! Detection is done by reading the volume descriptor set at sector 16 and,
//! when needed, walking the ISO 9660 directory tree. This keeps the image list
//! useful even when the optional `xorriso` conversion tool is not installed.
//!
//! Background: some re-packed Windows ISO images ship an empty ISO 9660 shell
//! (often a single `README.TXT`) while all real files live in the UDF
//! partition. Windows does not read UDF through a virtual USB CD-ROM drive, so
//! such an image cannot be mounted usefully until it is re-mastered as
//! ISO 9660 + Joliet.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// ISO 9660 trees smaller than this are treated as an empty shell. This matches
/// the `onekvm-iso-fix.sh` heuristic.
pub const ISO_SHELL_MIN_FILES: u32 = 100;

const SECTOR_SIZE: u64 = 2048;
const VOLUME_DESCRIPTOR_START_SECTOR: u64 = 16;
const VOLUME_DESCRIPTOR_SCAN_SECTORS: u64 = 64;
const PVD_ROOT_RECORD_OFFSET: usize = 156;
const DIRECTORY_RECORD_MIN_LEN: usize = 34;
const DIRECTORY_RECORD_NAME_OFFSET: usize = 33;
const MAX_DIRECTORY_DEPTH: u32 = 32;
const MAX_VISITED_DIRECTORIES: u32 = 2048;
const MAX_DIRECTORY_BYTES: usize = 256 * 1024;
const MAX_TOTAL_DIRECTORY_BYTES: usize = 8 * 1024 * 1024;

const ISO_DESCRIPTOR_ID: &[u8] = b"CD001";
const JOLIET_ESCAPES: [&[u8]; 3] = [b"%/@", b"%/C", b"%/E"];
const EL_TORITO_ID: &[u8] = b"EL TORITO SPECIFICATION";
const UDF_VRS_IDS: [&[u8]; 3] = [b"BEA01", b"NSR02", b"NSR03"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsoFormat {
    /// Readable by Windows through the virtual CD-ROM drive.
    Ok,
    /// Full ISO 9660 tree, but no Joliet directory.
    MissingJoliet,
    /// Empty ISO 9660 shell whose content lives in the UDF partition.
    UdfOnly,
    /// No usable ISO 9660 or UDF content; cannot be converted.
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoCompatibility {
    pub format: IsoFormat,
    pub needs_conversion: bool,
    pub convertible: bool,
    pub has_joliet: bool,
    pub has_udf: bool,
    pub has_el_torito: bool,
    /// ISO 9660 file count, capped at [`ISO_SHELL_MIN_FILES`].
    pub iso9660_files: Option<u32>,
    /// True when the walk stopped at the cap or hit a safety limit.
    pub file_count_truncated: bool,
}

impl IsoCompatibility {
    pub fn unsupported() -> Self {
        Self {
            format: IsoFormat::Unsupported,
            needs_conversion: false,
            convertible: false,
            has_joliet: false,
            has_udf: false,
            has_el_torito: false,
            iso9660_files: None,
            file_count_truncated: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IsoProbe {
    pub compatibility: IsoCompatibility,
    pub volume_label: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct DirectoryExtent {
    sector: u32,
    extended_attribute_sectors: u32,
    length: u32,
}

impl DirectoryExtent {
    fn data_sector(self) -> u64 {
        u64::from(self.sector) + u64::from(self.extended_attribute_sectors)
    }
}

#[derive(Debug, Default)]
struct VolumeDescriptorInfo {
    root: Option<DirectoryExtent>,
    has_joliet: bool,
    has_udf: bool,
    has_el_torito: bool,
    volume_label: Option<String>,
}

/// Probe an image without mounting it.
///
/// Only I/O errors opening the file are returned; malformed or non-ISO data is
/// reported as [`IsoFormat::Unsupported`].
pub fn probe_iso(path: &Path) -> std::io::Result<IsoProbe> {
    let mut file = File::open(path)?;
    let descriptor = scan_volume_descriptors(&mut file);

    let (file_count, file_count_truncated) = match descriptor.root {
        Some(root) => match count_iso9660_files(&mut file, root) {
            Ok((count, truncated)) => {
                // Hitting the cap means the tree may contain more files than
                // we counted, so treat it as truncated even when the count is
                // exactly the cap.
                (Some(count), truncated || count >= ISO_SHELL_MIN_FILES)
            }
            Err(_) => (None, false),
        },
        None => (None, false),
    };

    let effective_files = match (file_count, file_count_truncated) {
        (Some(count), true) => count.max(ISO_SHELL_MIN_FILES),
        (Some(count), false) => count,
        (None, _) => 0,
    };
    let has_content = effective_files >= ISO_SHELL_MIN_FILES;

    let (format, needs_conversion, convertible) = if !has_content {
        if descriptor.has_udf {
            (IsoFormat::UdfOnly, true, true)
        } else if descriptor.has_joliet {
            (IsoFormat::Ok, false, false)
        } else {
            (IsoFormat::Unsupported, false, false)
        }
    } else if !descriptor.has_joliet {
        (IsoFormat::MissingJoliet, true, true)
    } else {
        (IsoFormat::Ok, false, false)
    };

    Ok(IsoProbe {
        compatibility: IsoCompatibility {
            format,
            needs_conversion,
            convertible,
            has_joliet: descriptor.has_joliet,
            has_udf: descriptor.has_udf,
            has_el_torito: descriptor.has_el_torito,
            iso9660_files: file_count,
            file_count_truncated,
        },
        volume_label: descriptor.volume_label,
    })
}

fn scan_volume_descriptors(file: &mut File) -> VolumeDescriptorInfo {
    let mut info = VolumeDescriptorInfo::default();

    for index in 0..VOLUME_DESCRIPTOR_SCAN_SECTORS {
        let Some(sector) = read_sector(file, VOLUME_DESCRIPTOR_START_SECTOR + index) else {
            break;
        };

        if &sector[1..6] == ISO_DESCRIPTOR_ID {
            match sector[0] {
                0 if sector[7..39].starts_with(EL_TORITO_ID) => {
                    info.has_el_torito = true;
                }
                1 => {
                    info.root = parse_root_extent(&sector[PVD_ROOT_RECORD_OFFSET..]);
                    info.volume_label = parse_volume_label(&sector);
                }
                2 if is_joliet_descriptor(&sector) => {
                    info.has_joliet = true;
                }
                _ => {}
            }
        } else if UDF_VRS_IDS
            .iter()
            .any(|identifier| &sector[1..6] == *identifier)
        {
            info.has_udf = true;
        }
    }

    info
}

fn read_sector(file: &mut File, sector: u64) -> Option<[u8; SECTOR_SIZE as usize]> {
    let mut buffer = [0u8; SECTOR_SIZE as usize];
    file.seek(SeekFrom::Start(sector * SECTOR_SIZE)).ok()?;
    file.read_exact(&mut buffer).ok()?;
    Some(buffer)
}

fn parse_root_extent(record: &[u8]) -> Option<DirectoryExtent> {
    parse_directory_extent(record)
}

fn parse_directory_extent(record: &[u8]) -> Option<DirectoryExtent> {
    if record.len() < DIRECTORY_RECORD_MIN_LEN {
        return None;
    }
    let length = u32::from_le_bytes(record[10..14].try_into().ok()?);
    if length == 0 {
        return None;
    }
    Some(DirectoryExtent {
        sector: u32::from_le_bytes(record[2..6].try_into().ok()?),
        extended_attribute_sectors: u32::from(record[1]),
        length,
    })
}

fn parse_volume_label(sector: &[u8; SECTOR_SIZE as usize]) -> Option<String> {
    let raw = &sector[40..72];
    let label: String = raw
        .iter()
        .map(|&byte| {
            if (0x20..=0x7e).contains(&byte) {
                byte as char
            } else {
                ' '
            }
        })
        .collect();
    let label = label.trim().to_string();
    if label.is_empty() {
        None
    } else {
        Some(label)
    }
}

fn is_joliet_descriptor(sector: &[u8; SECTOR_SIZE as usize]) -> bool {
    JOLIET_ESCAPES
        .iter()
        .any(|escape| sector.get(88..91) == Some(*escape))
}

fn count_iso9660_files(file: &mut File, root: DirectoryExtent) -> std::io::Result<(u32, bool)> {
    let mut counter = FileCounter::default();
    counter.count_directory(file, root, 0)?;
    Ok((counter.files, counter.truncated))
}

#[derive(Default)]
struct FileCounter {
    files: u32,
    visited_directories: u32,
    read_bytes: usize,
    truncated: bool,
    visited_extents: HashSet<(u32, u32)>,
}

impl FileCounter {
    fn count_directory(
        &mut self,
        file: &mut File,
        extent: DirectoryExtent,
        depth: u32,
    ) -> std::io::Result<()> {
        if self.reached_limit() {
            return Ok(());
        }
        if depth > MAX_DIRECTORY_DEPTH
            || self.visited_directories >= MAX_VISITED_DIRECTORIES
            || self.read_bytes >= MAX_TOTAL_DIRECTORY_BYTES
        {
            self.truncated = true;
            return Ok(());
        }
        if !self.visited_extents.insert((extent.sector, extent.length)) {
            return Ok(());
        }
        self.visited_directories += 1;

        let declared_length = extent.length as usize;
        if declared_length > MAX_DIRECTORY_BYTES {
            self.truncated = true;
        }
        let length = declared_length.min(MAX_DIRECTORY_BYTES);
        self.read_bytes = self.read_bytes.saturating_add(length);
        let mut data = vec![0u8; length];
        file.seek(SeekFrom::Start(extent.data_sector() * SECTOR_SIZE))?;
        file.read_exact(&mut data)?;

        let mut position = 0usize;
        while position < data.len() && !self.reached_limit() {
            let record_len = data[position] as usize;
            if record_len == 0 {
                position = next_sector_boundary(position);
                continue;
            }
            if record_len < DIRECTORY_RECORD_MIN_LEN || position + record_len > data.len() {
                break;
            }

            let record = &data[position..position + record_len];
            position += record_len;

            let name_len = record[32] as usize;
            if name_len == 0 || DIRECTORY_RECORD_NAME_OFFSET + name_len > record.len() {
                continue;
            }
            let name =
                &record[DIRECTORY_RECORD_NAME_OFFSET..DIRECTORY_RECORD_NAME_OFFSET + name_len];
            if name.len() == 1 && (name[0] == 0 || name[0] == 1) {
                continue;
            }

            if record[25] & 0x02 != 0 {
                if let Some(child) = parse_directory_extent(record) {
                    self.count_directory(file, child, depth + 1)?;
                }
            } else {
                self.files += 1;
            }
        }

        Ok(())
    }

    fn reached_limit(&self) -> bool {
        self.files >= ISO_SHELL_MIN_FILES || self.truncated
    }
}

fn next_sector_boundary(position: usize) -> usize {
    let sector = SECTOR_SIZE as usize;
    (position / sector + 1) * sector
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const SECTOR: usize = SECTOR_SIZE as usize;
    const ROOT_SECTOR: u32 = 20;

    fn empty_image() -> Vec<u8> {
        vec![0u8; 32 * SECTOR]
    }

    fn write_sector(image: &mut [u8], sector: u64, content: &[u8; SECTOR]) {
        let offset = sector as usize * SECTOR;
        image[offset..offset + SECTOR].copy_from_slice(content);
    }

    fn directory_record(name: &[u8], extent: u32, length: u32, is_dir: bool) -> Vec<u8> {
        let mut record_len = DIRECTORY_RECORD_NAME_OFFSET + name.len();
        if !record_len.is_multiple_of(2) {
            record_len += 1;
        }
        let mut record = vec![0u8; record_len];
        record[0] = record_len as u8;
        record[2..6].copy_from_slice(&extent.to_le_bytes());
        record[6..10].copy_from_slice(&extent.to_be_bytes());
        record[10..14].copy_from_slice(&length.to_le_bytes());
        record[14..18].copy_from_slice(&length.to_be_bytes());
        record[25] = if is_dir { 0x02 } else { 0 };
        record[32] = name.len() as u8;
        record[DIRECTORY_RECORD_NAME_OFFSET..DIRECTORY_RECORD_NAME_OFFSET + name.len()]
            .copy_from_slice(name);
        record
    }

    fn root_record(root_len: u32) -> Vec<u8> {
        directory_record(&[0], ROOT_SECTOR, root_len, true)
    }

    fn primary_descriptor(label: &str, root_len: u32) -> [u8; SECTOR] {
        let mut sector = [0u8; SECTOR];
        sector[0] = 1;
        sector[1..6].copy_from_slice(ISO_DESCRIPTOR_ID);
        sector[6] = 1;
        let mut label_bytes = [b' '; 32];
        let label = label.as_bytes();
        let len = label.len().min(label_bytes.len());
        label_bytes[..len].copy_from_slice(&label[..len]);
        sector[40..72].copy_from_slice(&label_bytes);
        let record = root_record(root_len);
        sector[PVD_ROOT_RECORD_OFFSET..PVD_ROOT_RECORD_OFFSET + record.len()]
            .copy_from_slice(&record);
        sector
    }

    fn joliet_descriptor(root_len: u32) -> [u8; SECTOR] {
        let mut sector = primary_descriptor("JOLIET", root_len);
        sector[0] = 2;
        sector[88..91].copy_from_slice(b"%/E");
        sector
    }

    fn boot_descriptor() -> [u8; SECTOR] {
        let mut sector = [0u8; SECTOR];
        sector[0] = 0;
        sector[1..6].copy_from_slice(ISO_DESCRIPTOR_ID);
        sector[7..7 + EL_TORITO_ID.len()].copy_from_slice(EL_TORITO_ID);
        sector
    }

    fn udf_vrs_descriptor(identifier: &[u8]) -> [u8; SECTOR] {
        let mut sector = [0u8; SECTOR];
        sector[1..6].copy_from_slice(identifier);
        sector
    }

    fn write_root_directory(image: &mut [u8], entries: &[Vec<u8>]) -> u32 {
        let mut directory = Vec::new();
        directory.extend(directory_record(&[0], ROOT_SECTOR, 0, true));
        directory.extend(directory_record(&[1], ROOT_SECTOR, 0, true));
        for entry in entries {
            directory.extend_from_slice(entry);
        }
        let padded_len = directory.len().div_ceil(SECTOR) * SECTOR;
        directory.resize(padded_len, 0);
        let offset = ROOT_SECTOR as usize * SECTOR;
        image[offset..offset + directory.len()].copy_from_slice(&directory);
        directory.len() as u32
    }

    fn write_file(path: &Path, image: &[u8]) {
        fs::write(path, image).unwrap();
    }

    #[test]
    fn reports_joliet_image_as_ok() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("windows.iso");
        let mut image = empty_image();
        write_sector(&mut image, 16, &primary_descriptor("WIN10", SECTOR as u32));
        write_sector(&mut image, 17, &joliet_descriptor(SECTOR as u32));
        write_sector(&mut image, 18, &boot_descriptor());
        write_root_directory(
            &mut image,
            &[directory_record(b"README.TXT", 21, 64, false)],
        );
        write_file(&path, &image);

        let probe = probe_iso(&path).unwrap();

        assert_eq!(probe.compatibility.format, IsoFormat::Ok);
        assert!(!probe.compatibility.needs_conversion);
        assert!(!probe.compatibility.convertible);
        assert!(probe.compatibility.has_joliet);
        assert!(!probe.compatibility.has_udf);
        assert!(probe.compatibility.has_el_torito);
        assert_eq!(probe.compatibility.iso9660_files, Some(1));
        assert_eq!(probe.volume_label.as_deref(), Some("WIN10"));
    }

    #[test]
    fn reports_udf_only_shell_as_convertible() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("udf-shell.iso");
        let mut image = empty_image();
        write_sector(
            &mut image,
            16,
            &primary_descriptor("UDF_SHELL", SECTOR as u32),
        );
        write_sector(&mut image, 17, &udf_vrs_descriptor(b"BEA01"));
        write_sector(&mut image, 18, &udf_vrs_descriptor(b"NSR02"));
        write_sector(&mut image, 19, &udf_vrs_descriptor(b"TEA01"));
        write_root_directory(
            &mut image,
            &[directory_record(b"README.TXT", 21, 64, false)],
        );
        write_file(&path, &image);

        let probe = probe_iso(&path).unwrap();

        assert_eq!(probe.compatibility.format, IsoFormat::UdfOnly);
        assert!(probe.compatibility.needs_conversion);
        assert!(probe.compatibility.convertible);
        assert!(!probe.compatibility.has_joliet);
        assert!(probe.compatibility.has_udf);
    }

    #[test]
    fn reports_full_iso9660_tree_without_joliet_as_convertible() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("no-joliet.iso");
        let mut image = empty_image();
        let entries: Vec<Vec<u8>> = (0..ISO_SHELL_MIN_FILES)
            .map(|index| {
                directory_record(
                    format!("FILE{index:03}.TXT").as_bytes(),
                    21 + index,
                    64,
                    false,
                )
            })
            .collect();
        write_sector(
            &mut image,
            16,
            &primary_descriptor("NOJOLIET", SECTOR as u32),
        );
        let root_len = write_root_directory(&mut image, &entries);
        let pvd = primary_descriptor("NOJOLIET", root_len);
        write_sector(&mut image, 16, &pvd);
        write_file(&path, &image);

        let probe = probe_iso(&path).unwrap();

        assert_eq!(probe.compatibility.format, IsoFormat::MissingJoliet);
        assert!(probe.compatibility.needs_conversion);
        assert!(probe.compatibility.convertible);
        assert_eq!(probe.compatibility.iso9660_files, Some(ISO_SHELL_MIN_FILES));
        assert!(probe.compatibility.file_count_truncated);
    }

    #[test]
    fn reports_unknown_data_as_unsupported() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("random.iso");
        write_file(&path, &empty_image());

        let probe = probe_iso(&path).unwrap();

        assert_eq!(probe.compatibility.format, IsoFormat::Unsupported);
        assert!(!probe.compatibility.needs_conversion);
        assert!(!probe.compatibility.convertible);
    }

    #[test]
    fn small_iso_with_joliet_and_no_udf_stays_ok() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("small.iso");
        let mut image = empty_image();
        write_sector(&mut image, 16, &primary_descriptor("SMALL", SECTOR as u32));
        write_sector(&mut image, 17, &joliet_descriptor(SECTOR as u32));
        write_root_directory(
            &mut image,
            &[directory_record(b"DRIVER.EXE", 21, 64, false)],
        );
        write_file(&path, &image);

        let probe = probe_iso(&path).unwrap();

        assert_eq!(probe.compatibility.format, IsoFormat::Ok);
    }

    #[test]
    fn missing_pvd_but_udf_is_convertible() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("pure-udf.iso");
        let mut image = empty_image();
        write_sector(&mut image, 16, &udf_vrs_descriptor(b"BEA01"));
        write_sector(&mut image, 17, &udf_vrs_descriptor(b"NSR03"));
        write_file(&path, &image);

        let probe = probe_iso(&path).unwrap();

        assert_eq!(probe.compatibility.format, IsoFormat::UdfOnly);
        assert!(probe.compatibility.convertible);
        assert_eq!(probe.compatibility.iso9660_files, None);
    }

    #[test]
    fn directory_records_can_span_sectors() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("multi-sector.iso");
        let mut image = empty_image();
        let entries: Vec<Vec<u8>> = (0..60)
            .map(|index| {
                directory_record(
                    format!("A_VERY_LONG_FILE_NAME_{index:03}.TXT").as_bytes(),
                    30 + index,
                    64,
                    false,
                )
            })
            .collect();
        write_sector(&mut image, 16, &primary_descriptor("MULTI", SECTOR as u32));
        let root_len = write_root_directory(&mut image, &entries);
        let pvd = primary_descriptor("MULTI", root_len);
        write_sector(&mut image, 16, &pvd);
        write_file(&path, &image);

        let probe = probe_iso(&path).unwrap();

        assert_eq!(probe.compatibility.iso9660_files, Some(60));
        assert_eq!(probe.compatibility.format, IsoFormat::Unsupported);
    }
}
