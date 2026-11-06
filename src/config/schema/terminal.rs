use serde::{Deserialize, Serialize};
use typeshare::typeshare;

/// Built-in browser terminal settings.
///
/// The built-in terminal does not depend on an external `ttyd` binary; the
/// server allocates a pseudo-terminal and runs the configured shell directly.
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    /// Whether authenticated users may open the built-in web terminal.
    pub enabled: bool,
    /// Shell executable used for new sessions. Arguments are allowed, for
    /// example `/bin/bash -l`. Arguments are passed directly to the process and
    /// are never interpreted by a shell.
    pub shell: String,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            // Opt-in: a web shell runs with the service account's privileges.
            enabled: false,
            shell: default_shell().to_string(),
        }
    }
}

/// Default shell for the current platform.
pub fn default_shell() -> &'static str {
    #[cfg(unix)]
    {
        if std::path::Path::new("/bin/bash").is_file() {
            "/bin/bash"
        } else {
            "/bin/sh"
        }
    }
    #[cfg(windows)]
    {
        "cmd"
    }
}

/// Validate a configured shell command without executing it.
///
/// Only the executable (the first whitespace-separated token) is checked. The
/// remaining tokens are passed as literal arguments, so there is no shell
/// injection surface here.
pub fn validate_shell(shell: &str) -> Result<(), String> {
    let shell = shell.trim();
    if shell.is_empty() {
        return Err("Shell path must not be empty".to_string());
    }

    let program = shell
        .split_whitespace()
        .next()
        .ok_or_else(|| "Shell path must not be empty".to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = std::path::Path::new(program);
        if !path.is_absolute() {
            return Err("Shell path must be absolute".to_string());
        }

        let metadata =
            std::fs::metadata(path).map_err(|_| format!("Shell not found: {program}"))?;
        if !metadata.is_file() {
            return Err(format!("Shell is not a regular file: {program}"));
        }
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!("Shell is not executable: {program}"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{default_shell, validate_shell, TerminalConfig};

    #[test]
    fn default_config_is_opt_in() {
        let config = TerminalConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.shell, default_shell());
    }

    #[test]
    fn missing_terminal_section_uses_defaults() {
        let config: TerminalConfig = serde_json::from_str("{}").unwrap();
        assert!(!config.enabled);
        assert_eq!(config.shell, default_shell());
    }

    #[test]
    fn rejects_empty_and_relative_shells() {
        assert!(validate_shell("").is_err());
        assert!(validate_shell("   ").is_err());
        #[cfg(unix)]
        assert!(validate_shell("bash").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn accepts_existing_executable() {
        assert!(validate_shell("/bin/sh").is_ok());
        assert!(validate_shell("/bin/sh -l").is_ok());
        assert!(validate_shell("/bin/sh --definitely-not-a-real-flag").is_ok());
    }
}
