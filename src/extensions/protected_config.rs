use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub(crate) struct ProtectedConfigFile {
    _temp_dir: TempDir,
    path: PathBuf,
}

impl ProtectedConfigFile {
    pub(crate) async fn create(
        extension_name: &str,
        file_name: &str,
        contents: &str,
    ) -> Result<Self, String> {
        let temp_dir = tempfile::tempdir().map_err(|error| {
            format!("Failed to create {} config dir: {}", extension_name, error)
        })?;
        let path = temp_dir.path().join(file_name);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o700))
                .map_err(|error| {
                    format!("Failed to protect {} config dir: {}", extension_name, error)
                })?;
        }

        tokio::fs::write(&path, contents)
            .await
            .map_err(|error| format!("Failed to write {} config: {}", extension_name, error))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .await
                .map_err(|error| {
                    format!("Failed to protect {} config: {}", extension_name, error)
                })?;
        }

        Ok(Self {
            _temp_dir: temp_dir,
            path,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn protects_and_cleans_up_config_file() {
        let config =
            ProtectedConfigFile::create("Test extension", "extension.toml", "enabled = true\n")
                .await
                .unwrap();
        let path = config.path().to_path_buf();

        assert_eq!(
            tokio::fs::read_to_string(&path).await.unwrap(),
            "enabled = true\n"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                std::fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        drop(config);
        assert!(!path.exists());
    }
}
