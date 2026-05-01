//! Host identity helpers.

/// Truncated content of `/etc/hostname`. Used where RustDesk peers expect the configured static name.
pub fn hostname_from_etc() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "One-KVM".to_string())
}

/// Current kernel hostname (`gethostname`). Used for live device info in the UI.
pub fn hostname_uname() -> String {
    nix::unistd::gethostname()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".to_string())
}
