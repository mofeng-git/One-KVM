use std::path::Path;

pub(super) fn has_boot_entry(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "-b" || arg == "-e")
}

pub(super) fn find_boot_options(root: &Path) -> Vec<String> {
    let mut entries: Vec<Vec<String>> = Vec::new();

    if let Some(path) = resolve_case_insensitive(root, &["boot", "etfsboot.com"]) {
        entries.push(boot_entry("-b", &path, Some("8")));
    } else if resolve_case_insensitive(root, &["boot", "bcd"]).is_some() {
        entries.push(boot_entry("-b", "boot/etfsboot.com", Some("8")));
    }

    let efi_image = resolve_case_insensitive(root, &["efi", "microsoft", "boot", "efisys.bin"])
        .or_else(|| {
            resolve_case_insensitive(root, &["efi", "microsoft", "boot", "efisys_noprompt.bin"])
        })
        .or_else(|| resolve_case_insensitive(root, &["efi", "boot", "efiboot.img"]));
    if let Some(path) = efi_image {
        entries.push(boot_entry("-e", &path, None));
    }

    if entries.is_empty() {
        if let Some(path) = resolve_case_insensitive(root, &["isolinux", "isolinux.bin"]) {
            entries.push(vec![
                "-b".to_string(),
                path,
                "-c".to_string(),
                "isolinux/boot.cat".to_string(),
                "-no-emul-boot".to_string(),
                "-boot-load-size".to_string(),
                "4".to_string(),
                "-boot-info-table".to_string(),
            ]);
        }
    } else if let Some(path) = resolve_case_insensitive(root, &["boot", "grub", "efi.img"]) {
        entries.push(boot_entry("-e", &path, None));
    }

    let mut args = Vec::new();
    for (index, entry) in entries.into_iter().enumerate() {
        if index > 0 {
            args.push("-eltorito-alt-boot".to_string());
        }
        args.extend(entry);
    }
    args
}

fn boot_entry(flag: &str, path: &str, boot_load_size: Option<&str>) -> Vec<String> {
    let mut entry = vec![
        flag.to_string(),
        path.to_string(),
        "-no-emul-boot".to_string(),
    ];
    if let Some(size) = boot_load_size {
        entry.push("-boot-load-size".to_string());
        entry.push(size.to_string());
    }
    entry
}

fn resolve_case_insensitive(root: &Path, parts: &[&str]) -> Option<String> {
    let mut current = root.to_path_buf();
    let mut resolved = Vec::with_capacity(parts.len());

    for (index, part) in parts.iter().enumerate() {
        let entry = std::fs::read_dir(&current)
            .ok()?
            .filter_map(|entry| entry.ok())
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(part)
            })?;
        let is_last = index + 1 == parts.len();
        let file_type = entry.file_type().ok()?;
        if is_last {
            if !file_type.is_file() {
                return None;
            }
        } else if !file_type.is_dir() {
            return None;
        }
        resolved.push(entry.file_name().to_string_lossy().to_string());
        current = entry.path();
    }

    Some(resolved.join("/"))
}

pub(super) fn parse_mkisofs_boot_args(stdout: &str) -> Vec<String> {
    let mut args = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if !line.starts_with('-') || line.starts_with("--") {
            continue;
        }
        args.extend(split_shell_like(line));
    }
    remove_volume_arguments(args)
}

fn remove_volume_arguments(args: Vec<String>) -> Vec<String> {
    let mut cleaned = Vec::with_capacity(args.len());
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "-V" || arg == "-volid" {
            skip_next = true;
            continue;
        }
        cleaned.push(arg);
    }
    cleaned
}

fn split_shell_like(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for character in line.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        match quote {
            Some(active) => {
                if character == active {
                    quote = None;
                } else {
                    current.push(character);
                }
            }
            None => {
                if character == '\'' || character == '"' {
                    quote = Some(character);
                } else if character.is_whitespace() {
                    if !current.is_empty() {
                        args.push(std::mem::take(&mut current));
                    }
                } else {
                    current.push(character);
                }
            }
        }
    }

    if !current.is_empty() {
        args.push(current);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detects_boot_entries() {
        assert!(has_boot_entry(&["-b".into(), "boot.bin".into()]));
        assert!(has_boot_entry(&["-e".into(), "efi.img".into()]));
        assert!(!has_boot_entry(&["-no-emul-boot".into()]));
    }

    #[test]
    fn finds_windows_boot_files_case_insensitively() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("BOOT")).unwrap();
        fs::create_dir_all(root.join("EFI/Microsoft/Boot")).unwrap();
        fs::write(root.join("BOOT/ETFSBOOT.COM"), b"boot").unwrap();
        fs::write(root.join("EFI/Microsoft/Boot/efisys.bin"), b"efi").unwrap();

        let args = find_boot_options(root);

        assert_eq!(
            args,
            vec![
                "-b",
                "BOOT/ETFSBOOT.COM",
                "-no-emul-boot",
                "-boot-load-size",
                "8",
                "-eltorito-alt-boot",
                "-e",
                "EFI/Microsoft/Boot/efisys.bin",
                "-no-emul-boot",
            ]
        );
    }

    #[test]
    fn falls_back_to_isolinux_when_no_windows_boot_files_exist() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("isolinux")).unwrap();
        fs::write(root.join("isolinux/isolinux.bin"), b"boot").unwrap();

        let args = find_boot_options(root);

        assert!(args.contains(&"-boot-info-table".to_string()));
        assert!(args.contains(&"isolinux/isolinux.bin".to_string()));
    }

    #[test]
    fn parses_mkisofs_catalog_arguments() {
        let stdout = "\
xorriso : NOTE : Some note
-b 'boot/etfsboot.com' -no-emul-boot -boot-load-size 8
-eltorito-alt-boot -e 'efi/microsoft/boot/efisys.bin' -no-emul-boot
-V 'MY_LABEL'
";
        let args = parse_mkisofs_boot_args(stdout);

        assert_eq!(
            args,
            vec![
                "-b",
                "boot/etfsboot.com",
                "-no-emul-boot",
                "-boot-load-size",
                "8",
                "-eltorito-alt-boot",
                "-e",
                "efi/microsoft/boot/efisys.bin",
                "-no-emul-boot",
            ]
        );
    }

    #[test]
    fn splits_single_and_double_quoted_arguments() {
        assert_eq!(
            split_shell_like("-a 'two words' -b \"three words\""),
            vec!["-a", "two words", "-b", "three words"]
        );
    }
}
