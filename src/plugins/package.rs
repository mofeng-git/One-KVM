use std::path::Path;

use one_kvm_plugin_sdk::{Manifest, ABI, MAGIC, MAX_MANIFEST, MAX_PACKAGE};
use tokio::{fs, io::AsyncReadExt};

use crate::{AppError, Result};

pub(super) const EXECUTABLE: &str = if cfg!(windows) {
    "plugin.exe"
} else {
    "plugin"
};

pub(super) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
        && !matches!(
            id,
            "con"
                | "prn"
                | "aux"
                | "nul"
                | "com1"
                | "com2"
                | "com3"
                | "com4"
                | "com5"
                | "com6"
                | "com7"
                | "com8"
                | "com9"
                | "lpt1"
                | "lpt2"
                | "lpt3"
                | "lpt4"
                | "lpt5"
                | "lpt6"
                | "lpt7"
                | "lpt8"
                | "lpt9"
        )
}

fn valid_fields<'a>(keys: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = std::collections::HashSet::new();
    keys.into_iter()
        .all(|key| valid_id(key) && seen.insert(key))
}

pub(super) fn validate(manifest: &Manifest) -> Result<()> {
    if manifest.abi != ABI {
        return Err(AppError::BadRequest("Unsupported plugin ABI".into()));
    }
    if manifest.os != std::env::consts::OS || manifest.arch != std::env::consts::ARCH {
        return Err(AppError::BadRequest(
            "Plugin targets a different platform".into(),
        ));
    }
    if !valid_id(&manifest.id)
        || !manifest.name.has_text()
        || !valid_id(manifest.menu.id())
        || manifest.version.trim().is_empty()
        || manifest.author.trim().is_empty()
        || !valid_fields(
            manifest
                .config_schema
                .iter()
                .map(|field| field.key.as_str()),
        )
        || !valid_fields(
            manifest
                .status_fields
                .iter()
                .map(|field| field.key.as_str()),
        )
        || !valid_fields(manifest.actions.iter().map(|action| action.id.as_str()))
        || manifest.actions.iter().any(|action| {
            !action.label.has_text()
                || action.when_status.iter().any(|condition| {
                    !manifest
                        .status_fields
                        .iter()
                        .any(|field| field.key == condition.key)
                })
        })
        || manifest
            .status_fields
            .iter()
            .any(|field| !field.label.has_text())
        || manifest.config_schema.iter().any(|field| {
            !field.label.has_text()
                || field
                    .choice_labels
                    .keys()
                    .any(|choice| !field.choices.contains(choice))
                || !field.accepts(&field.default)
                || field.when.as_ref().is_some_and(|condition| {
                    field.key == condition.key
                        || !manifest.config_schema.iter().any(|controller| {
                            controller.key == condition.key && controller.accepts(&condition.equals)
                        })
                })
        })
    {
        return Err(AppError::BadRequest("Invalid plugin manifest".into()));
    }
    Ok(())
}

pub(super) async fn unpack(source: &Path, destination: &Path) -> Result<Manifest> {
    let mut source = fs::File::open(source).await?;
    if source.metadata().await?.len() > MAX_PACKAGE as u64 {
        return Err(AppError::BadRequest(
            "Plugin package exceeds the size limit".into(),
        ));
    }
    let mut magic = vec![0; MAGIC.len()];
    if source.read_exact(&mut magic).await.is_err() || magic != MAGIC {
        return Err(AppError::BadRequest("Invalid .onepkg header".into()));
    }
    let length = source.read_u32().await.map_err(incomplete)? as usize;
    if length == 0 || length > MAX_MANIFEST {
        return Err(AppError::BadRequest(
            "Invalid plugin manifest length".into(),
        ));
    }
    let mut json = vec![0; length];
    source.read_exact(&mut json).await.map_err(incomplete)?;
    let manifest: Manifest = serde_json::from_slice(&json)
        .map_err(|error| AppError::BadRequest(format!("Invalid plugin manifest: {error}")))?;
    validate(&manifest)?;

    let mut executable = fs::OpenOptions::new();
    executable.write(true).create_new(true);
    #[cfg(unix)]
    executable.mode(0o700);
    let mut executable = executable.open(destination.join(EXECUTABLE)).await?;
    if tokio::io::copy(&mut source, &mut executable).await? == 0 {
        return Err(AppError::BadRequest(
            "Plugin package has no executable".into(),
        ));
    }
    executable.sync_all().await?;
    write_json(&destination.join("manifest.json"), &manifest).await?;
    Ok(manifest)
}

fn incomplete(error: std::io::Error) -> AppError {
    if error.kind() == std::io::ErrorKind::UnexpectedEof {
        AppError::BadRequest("Incomplete .onepkg manifest".into())
    } else {
        error.into()
    }
}

pub(super) async fn write_json(path: &Path, data: &impl serde::Serialize) -> Result<()> {
    let file = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    let mut writer = fs::File::from_std(file.reopen()?);
    use tokio::io::AsyncWriteExt;
    writer.write_all(&serde_json::to_vec(data)?).await?;
    writer.sync_all().await?;
    drop(writer);
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_ids_are_safe_path_components() {
        for id in [
            "",
            "..",
            "../plugin",
            "x/y",
            "x\\y",
            "D:",
            "COM1",
            "con",
            "dashboard.json",
        ] {
            assert!(!valid_id(id), "{id}");
        }
        assert!(valid_id("other-service_2"));
    }
}
