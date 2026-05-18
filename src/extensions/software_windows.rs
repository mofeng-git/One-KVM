use std::path::PathBuf;

use super::ExtensionId;

pub fn default_binary_path(id: ExtensionId) -> &'static str {
    match id {
        ExtensionId::Ttyd => "ttyd.win32.exe",
        ExtensionId::Gostc => "gostc.exe",
        ExtensionId::Easytier => "easytier-core.exe",
    }
}

pub fn binary_path(id: ExtensionId) -> PathBuf {
    if id == ExtensionId::Ttyd {
        if let Some(path) = env_path("ONE_KVM_TTYD_PATH") {
            return path;
        }
    }

    find_in_app_dir(default_binary_path(id))
        .unwrap_or_else(|| PathBuf::from(default_binary_path(id)))
}

pub fn default_ttyd_shell() -> &'static str {
    "cmd"
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var(name)
        .ok()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

fn find_in_app_dir(binary_name: &str) -> Option<PathBuf> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let bundled = exe_dir.join(binary_name);
            if bundled.exists() {
                return Some(bundled);
            }
        }
    }

    None
}
