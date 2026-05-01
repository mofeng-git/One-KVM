//! Small filesystem helpers.

use std::path::Path;

/// Read a UTF-8 file and trim surrounding whitespace.
pub fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}

/// Sorted list of directory entry names (lossy exclusion on non-UTF8).
pub fn list_dir_names(path: &Path) -> Vec<String> {
    let mut names = std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    names.sort();
    names
}
