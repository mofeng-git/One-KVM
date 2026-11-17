use std::path::Path;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::process::{cancelled_error, run_simple, EL_TORITO_TIMEOUT};
use crate::error::{AppError, MsdErrorCode, Result};
use crate::msd::iso::{probe_iso, IsoProbe, ISO_SHELL_MIN_FILES};

const MAX_TREE_DIRECTORIES: usize = 4096;

pub(super) async fn verify_output(
    part_path: &Path,
    source_path: &Path,
    required_files: u32,
    cancel: &CancellationToken,
) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(cancelled_error());
    }

    let output = probe_image(part_path).await?;
    let files = output.compatibility.iso9660_files.unwrap_or(0);
    let required_files = required_files.min(ISO_SHELL_MIN_FILES).max(1);
    if !output.compatibility.has_joliet || files < required_files {
        warn!(
            files,
            required_files,
            joliet = output.compatibility.has_joliet,
            "Converted ISO failed structural verification"
        );
        return Err(MsdErrorCode::MsdIsoConversionFailed.into());
    }

    if let (Some(source_counts), Some(output_counts)) = (
        el_torito_counts(source_path, cancel).await,
        el_torito_counts(part_path, cancel).await,
    ) {
        if output_counts.0 < source_counts.0 || output_counts.1 < source_counts.1 {
            warn!(
                source_bios = source_counts.0,
                source_efi = source_counts.1,
                new_bios = output_counts.0,
                new_efi = output_counts.1,
                "Converted ISO lost an El Torito boot entry"
            );
            return Err(MsdErrorCode::MsdIsoConversionFailed.into());
        }
    }

    Ok(())
}

async fn el_torito_counts(path: &Path, cancel: &CancellationToken) -> Option<(usize, usize)> {
    let mut command = Command::new("xorriso");
    command
        .arg("-indev")
        .arg(path)
        .args(["-report_el_torito", "plain"]);
    let output = run_simple(command, cancel, EL_TORITO_TIMEOUT).await.ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Some((
        text.lines().filter(|line| line.contains("BIOS")).count(),
        text.lines().filter(|line| line.contains("UEFI")).count(),
    ))
}

pub(super) async fn probe_image(path: &Path) -> Result<IsoProbe> {
    let path = path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || probe_iso(&path))
        .await
        .map_err(|error| {
            warn!(%error, "ISO probe task failed");
            AppError::from(MsdErrorCode::MsdOperationFailed)
        })?;
    result.map_err(|error| {
        warn!(%error, "Failed to probe ISO image");
        if error.kind() == std::io::ErrorKind::NotFound {
            MsdErrorCode::MsdResourceNotFound.into()
        } else {
            MsdErrorCode::MsdIsoConversionUnsupported.into()
        }
    })
}

pub(super) fn count_mounted_files(root: &Path) -> (u32, bool) {
    let mut stack = vec![root.to_path_buf()];
    let mut visited_directories = 0usize;
    let mut files = 0u32;

    while let Some(directory) = stack.pop() {
        visited_directories += 1;
        if visited_directories > MAX_TREE_DIRECTORIES {
            return (files, true);
        }

        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                files += 1;
                if files >= ISO_SHELL_MIN_FILES {
                    return (files, true);
                }
            }
        }
    }

    (files, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn counts_files_in_mounted_tree_and_stops_at_cap() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("sources")).unwrap();
        fs::write(root.join("README.TXT"), b"readme").unwrap();
        fs::write(root.join("sources/install.wim"), b"wim").unwrap();

        let (count, truncated) = count_mounted_files(root);
        assert_eq!(count, 2);
        assert!(!truncated);

        for index in 0..(ISO_SHELL_MIN_FILES + 5) {
            fs::write(root.join(format!("file{index}.dat")), b"x").unwrap();
        }
        let (count, truncated) = count_mounted_files(root);
        assert_eq!(count, ISO_SHELL_MIN_FILES);
        assert!(truncated);
    }

    #[test]
    fn empty_mounted_tree_has_no_files() {
        let temp = TempDir::new().unwrap();
        let (count, truncated) = count_mounted_files(temp.path());
        assert_eq!(count, 0);
        assert!(!truncated);
    }
}
