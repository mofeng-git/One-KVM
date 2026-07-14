use serde::Serialize;

use crate::utils::{list_dir_names, read_trimmed};

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OtgSelfCheckLevel {
    Info,
    Warn,
    Error,
}

#[derive(Serialize)]
pub struct OtgSelfCheckItem {
    pub id: &'static str,
    pub ok: bool,
    pub level: OtgSelfCheckLevel,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Serialize)]
pub struct OtgSelfCheckResponse {
    pub overall_ok: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub hid_backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_udc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_udc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udc_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udc_speed: Option<String>,
    pub available_udcs: Vec<String>,
    pub other_gadgets: Vec<String>,
    pub checks: Vec<OtgSelfCheckItem>,
}

fn push_otg_check(
    checks: &mut Vec<OtgSelfCheckItem>,
    id: &'static str,
    ok: bool,
    level: OtgSelfCheckLevel,
    message: impl Into<String>,
    hint: Option<impl Into<String>>,
    path: Option<impl Into<String>>,
) {
    checks.push(OtgSelfCheckItem {
        id,
        ok,
        level,
        message: message.into(),
        hint: hint.map(|v| v.into()),
        path: path.map(|v| v.into()),
    });
}

fn proc_modules_has(module_name: &str) -> bool {
    std::fs::read_to_string("/proc/modules")
        .ok()
        .map(|content| {
            content
                .lines()
                .filter_map(|line| line.split_whitespace().next())
                .any(|name| name == module_name)
        })
        .unwrap_or(false)
}

fn modules_metadata_has(module_name: &str) -> bool {
    let kernel_release = match read_trimmed(std::path::Path::new("/proc/sys/kernel/osrelease")) {
        Some(value) if !value.is_empty() => value,
        _ => return false,
    };

    let module_dir = std::path::Path::new("/lib/modules").join(kernel_release);
    let candidates = ["modules.builtin", "modules.builtin.modinfo", "modules.dep"];

    candidates.iter().any(|filename| {
        let path = module_dir.join(filename);
        std::fs::read_to_string(path)
            .ok()
            .map(|content| {
                let module_token = format!("/{module_name}.ko");
                content.lines().any(|line| {
                    line.contains(&module_token)
                        || line.contains(module_name)
                        || line.contains(&module_name.replace('_', "-"))
                })
            })
            .unwrap_or(false)
    })
}

fn kernel_config_option_enabled(option_name: &str) -> bool {
    let kernel_release = match read_trimmed(std::path::Path::new("/proc/sys/kernel/osrelease")) {
        Some(value) if !value.is_empty() => value,
        _ => return false,
    };

    let config_paths = [
        std::path::PathBuf::from(format!("/boot/config-{kernel_release}")),
        std::path::PathBuf::from("/boot/config"),
        std::path::PathBuf::from(format!("/lib/modules/{kernel_release}/build/.config")),
    ];

    config_paths.iter().any(|path| {
        std::fs::read_to_string(path)
            .ok()
            .map(|content| {
                let enabled_y = format!("{option_name}=y");
                let enabled_m = format!("{option_name}=m");
                content
                    .lines()
                    .any(|line| line == enabled_y || line == enabled_m)
            })
            .unwrap_or(false)
    })
}

fn detect_libcomposite_available(gadget_root: &std::path::Path) -> bool {
    let sys_module = std::path::Path::new("/sys/module/libcomposite").exists();
    if sys_module {
        return true;
    }

    if proc_modules_has("libcomposite") {
        return true;
    }

    if modules_metadata_has("libcomposite") {
        return true;
    }

    if kernel_config_option_enabled("CONFIG_USB_LIBCOMPOSITE")
        || kernel_config_option_enabled("CONFIG_USB_CONFIGFS")
    {
        return true;
    }

    // Fallback: if usb_gadget path exists, libcomposite may be built-in and already active.
    gadget_root.exists()
}

/// OTG self-check status for troubleshooting USB gadget issues
pub fn run(config: &crate::config::AppConfig) -> OtgSelfCheckResponse {
    let hid_backend_is_otg = matches!(config.hid.backend, crate::config::HidBackend::Otg);
    let mut checks = Vec::new();

    let build_response = |checks: Vec<OtgSelfCheckItem>,
                          selected_udc: Option<String>,
                          bound_udc: Option<String>,
                          udc_state: Option<String>,
                          udc_speed: Option<String>,
                          available_udcs: Vec<String>,
                          other_gadgets: Vec<String>| {
        let error_count = checks
            .iter()
            .filter(|item| item.level == OtgSelfCheckLevel::Error)
            .count();
        let warning_count = checks
            .iter()
            .filter(|item| item.level == OtgSelfCheckLevel::Warn)
            .count();

        OtgSelfCheckResponse {
            overall_ok: error_count == 0,
            error_count,
            warning_count,
            hid_backend: format!("{:?}", config.hid.backend).to_lowercase(),
            selected_udc,
            bound_udc,
            udc_state,
            udc_speed,
            available_udcs,
            other_gadgets,
            checks,
        }
    };

    let udc_root = std::path::Path::new("/sys/class/udc");
    let available_udcs = list_dir_names(udc_root);
    let selected_udc = config
        .hid
        .otg_udc
        .clone()
        .filter(|udc| !udc.trim().is_empty())
        .or_else(|| available_udcs.first().cloned());
    let mut udc_stage_ok = true;
    if !udc_root.exists() {
        udc_stage_ok = false;
        push_otg_check(
            &mut checks,
            "udc_dir_exists",
            false,
            OtgSelfCheckLevel::Error,
            "Check /sys/class/udc existence",
            Some("Ensure UDC/OTG kernel drivers are enabled"),
            Some("/sys/class/udc"),
        );
    } else if available_udcs.is_empty() {
        udc_stage_ok = false;
        push_otg_check(
            &mut checks,
            "udc_has_entries",
            false,
            OtgSelfCheckLevel::Error,
            "Check available UDC entries",
            Some("Ensure OTG controller is enabled in device tree"),
            Some("/sys/class/udc"),
        );
    } else {
        push_otg_check(
            &mut checks,
            "udc_has_entries",
            true,
            OtgSelfCheckLevel::Info,
            "Check available UDC entries",
            None::<String>,
            Some("/sys/class/udc"),
        );
    }

    let mut configured_udc_ok = true;
    if let Some(config_udc) = config
        .hid
        .otg_udc
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        if available_udcs.iter().any(|item| item == &config_udc) {
            push_otg_check(
                &mut checks,
                "configured_udc_valid",
                true,
                OtgSelfCheckLevel::Info,
                "Check configured UDC validity",
                None::<String>,
                Some("/sys/class/udc"),
            );
        } else {
            configured_udc_ok = false;
            push_otg_check(
                &mut checks,
                "configured_udc_valid",
                false,
                OtgSelfCheckLevel::Error,
                "Check configured UDC validity",
                Some("Please reselect UDC in HID OTG settings"),
                Some("/sys/class/udc"),
            );
        }
    } else {
        push_otg_check(
            &mut checks,
            "configured_udc_valid",
            !available_udcs.is_empty(),
            if available_udcs.is_empty() {
                OtgSelfCheckLevel::Warn
            } else {
                OtgSelfCheckLevel::Info
            },
            "Check configured UDC validity",
            Some(
                "You can set hid_otg_udc in settings to avoid ambiguity in multi-controller setups",
            ),
            Some("/sys/class/udc"),
        );
    }

    if !udc_stage_ok || !configured_udc_ok {
        return build_response(
            checks,
            selected_udc,
            None,
            None,
            None,
            available_udcs,
            vec![],
        );
    }

    let gadget_root = crate::otg::configfs::configfs_path();
    let configfs_mount = gadget_root
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/sys/kernel/config"));
    let configfs_mounted = std::fs::read_to_string("/proc/mounts")
        .ok()
        .map(|mounts| {
            mounts.lines().any(|line| {
                let mut parts = line.split_whitespace();
                let _src = parts.next();
                let mount_point = parts.next();
                let fs_type = parts.next();
                mount_point == configfs_mount.to_str() && fs_type == Some("configfs")
            })
        })
        .unwrap_or(false);

    let mut gadget_config_ok = true;

    if configfs_mounted {
        push_otg_check(
            &mut checks,
            "configfs_mounted",
            true,
            OtgSelfCheckLevel::Info,
            "Check configfs mount status",
            None::<String>,
            Some(configfs_mount.display().to_string()),
        );
    } else {
        gadget_config_ok = false;
        push_otg_check(
            &mut checks,
            "configfs_mounted",
            false,
            OtgSelfCheckLevel::Error,
            "Check configfs mount status",
            Some(format!(
                "Try: mount -t configfs none {}",
                configfs_mount.display()
            )),
            Some(configfs_mount.display().to_string()),
        );
    }

    if gadget_root.exists() {
        push_otg_check(
            &mut checks,
            "usb_gadget_dir_exists",
            true,
            OtgSelfCheckLevel::Info,
            format!("Check {} access", gadget_root.display()),
            None::<String>,
            Some(gadget_root.display().to_string()),
        );
    } else {
        gadget_config_ok = false;
        push_otg_check(
            &mut checks,
            "usb_gadget_dir_exists",
            false,
            OtgSelfCheckLevel::Error,
            format!("Check {} access", gadget_root.display()),
            Some("Ensure configfs and USB gadget support are enabled"),
            Some(gadget_root.display().to_string()),
        );
    }

    let libcomposite_available = detect_libcomposite_available(gadget_root);
    if libcomposite_available {
        push_otg_check(
            &mut checks,
            "libcomposite_loaded",
            true,
            OtgSelfCheckLevel::Info,
            "Check libcomposite module status",
            None::<String>,
            Some("/sys/module/libcomposite"),
        );
    } else {
        gadget_config_ok = false;
        push_otg_check(
            &mut checks,
            "libcomposite_loaded",
            false,
            OtgSelfCheckLevel::Error,
            "Check libcomposite module status",
            Some("Try: modprobe libcomposite"),
            Some("/sys/module/libcomposite"),
        );
    }

    if !gadget_config_ok {
        return build_response(
            checks,
            selected_udc,
            None,
            None,
            None,
            available_udcs,
            vec![],
        );
    }

    let gadget_names = list_dir_names(gadget_root);
    let one_kvm_path = gadget_root.join("one-kvm");
    let one_kvm_exists = one_kvm_path.exists();
    if one_kvm_exists {
        push_otg_check(
            &mut checks,
            "one_kvm_gadget_exists",
            true,
            OtgSelfCheckLevel::Info,
            "Check one-kvm gadget presence",
            None::<String>,
            Some(one_kvm_path.display().to_string()),
        );
    } else {
        push_otg_check(
            &mut checks,
            "one_kvm_gadget_exists",
            false,
            if hid_backend_is_otg {
                OtgSelfCheckLevel::Error
            } else {
                OtgSelfCheckLevel::Warn
            },
            "Check one-kvm gadget presence",
            Some("Enable OTG HID or MSD to let one-kvm gadget be created automatically"),
            Some(one_kvm_path.display().to_string()),
        );
    }

    let other_gadgets = gadget_names
        .iter()
        .filter(|name| name.as_str() != "one-kvm")
        .cloned()
        .collect::<Vec<_>>();
    if other_gadgets.is_empty() {
        push_otg_check(
            &mut checks,
            "other_gadgets",
            true,
            OtgSelfCheckLevel::Info,
            "Check for other gadget services",
            None::<String>,
            Some(gadget_root.display().to_string()),
        );
    } else {
        push_otg_check(
            &mut checks,
            "other_gadgets",
            false,
            OtgSelfCheckLevel::Warn,
            "Check for other gadget services",
            Some("Potential UDC contention with one-kvm; check other OTG services"),
            Some(gadget_root.display().to_string()),
        );
    }

    let mut bound_udc = None;

    if one_kvm_exists {
        let one_kvm_udc_path = one_kvm_path.join("UDC");
        let current_udc = read_trimmed(&one_kvm_udc_path).unwrap_or_default();
        if current_udc.is_empty() {
            push_otg_check(
                &mut checks,
                "one_kvm_bound_udc",
                false,
                OtgSelfCheckLevel::Warn,
                "Check one-kvm UDC binding",
                Some("Ensure HID/MSD is enabled and initialized successfully"),
                Some(one_kvm_udc_path.display().to_string()),
            );
        } else {
            push_otg_check(
                &mut checks,
                "one_kvm_bound_udc",
                true,
                OtgSelfCheckLevel::Info,
                "Check one-kvm UDC binding",
                None::<String>,
                Some(one_kvm_udc_path.display().to_string()),
            );
            bound_udc = Some(current_udc);
        }

        let functions_path = one_kvm_path.join("functions");
        let function_names = list_dir_names(&functions_path)
            .into_iter()
            .filter(|name| name.contains(".usb"))
            .collect::<Vec<_>>();
        let hid_functions = function_names
            .iter()
            .filter(|name| name.starts_with("hid.usb"))
            .cloned()
            .collect::<Vec<_>>();
        if hid_functions.is_empty() {
            push_otg_check(
                &mut checks,
                "hid_functions_present",
                false,
                if hid_backend_is_otg {
                    OtgSelfCheckLevel::Error
                } else {
                    OtgSelfCheckLevel::Warn
                },
                "Check HID function creation",
                Some("Check OTG HID config and enable at least one HID function"),
                Some(functions_path.display().to_string()),
            );
        } else {
            push_otg_check(
                &mut checks,
                "hid_functions_present",
                true,
                OtgSelfCheckLevel::Info,
                "Check HID function creation",
                None::<String>,
                Some(functions_path.display().to_string()),
            );
        }

        let config_path = one_kvm_path.join("configs/c.1");
        if !config_path.exists() {
            push_otg_check(
                &mut checks,
                "config_c1_exists",
                false,
                OtgSelfCheckLevel::Error,
                "Check configs/c.1 structure",
                Some("Gadget structure is incomplete; try restarting One-KVM"),
                Some(config_path.display().to_string()),
            );
        } else {
            push_otg_check(
                &mut checks,
                "config_c1_exists",
                true,
                OtgSelfCheckLevel::Info,
                "Check configs/c.1 structure",
                None::<String>,
                Some(config_path.display().to_string()),
            );

            let linked_functions = list_dir_names(&config_path)
                .into_iter()
                .filter(|name| name.contains(".usb"))
                .collect::<Vec<_>>();
            let missing_links = function_names
                .iter()
                .filter(|func| !linked_functions.iter().any(|link| link == *func))
                .cloned()
                .collect::<Vec<_>>();

            if missing_links.is_empty() {
                push_otg_check(
                    &mut checks,
                    "function_links_ok",
                    true,
                    OtgSelfCheckLevel::Info,
                    "Check function links in configs/c.1",
                    None::<String>,
                    Some(config_path.display().to_string()),
                );
            } else {
                push_otg_check(
                    &mut checks,
                    "function_links_ok",
                    false,
                    OtgSelfCheckLevel::Warn,
                    "Check function links in configs/c.1",
                    Some("Reinitialize OTG (toggle HID backend once or restart service)"),
                    Some(config_path.display().to_string()),
                );
            }
        }

        let missing_hid_devices = hid_functions
            .iter()
            .filter_map(|name| {
                let index = name.strip_prefix("hid.usb")?.parse::<u8>().ok()?;
                let dev_path = std::path::PathBuf::from(format!("/dev/hidg{}", index));
                if dev_path.exists() {
                    None
                } else {
                    Some(dev_path.display().to_string())
                }
            })
            .collect::<Vec<_>>();

        if !hid_functions.is_empty() {
            if missing_hid_devices.is_empty() {
                push_otg_check(
                    &mut checks,
                    "hid_device_nodes",
                    true,
                    OtgSelfCheckLevel::Info,
                    "Check /dev/hidg* device nodes",
                    None::<String>,
                    Some("/dev/hidg*"),
                );
            } else {
                push_otg_check(
                    &mut checks,
                    "hid_device_nodes",
                    false,
                    OtgSelfCheckLevel::Warn,
                    "Check /dev/hidg* device nodes",
                    Some("Ensure gadget is bound and check kernel logs"),
                    Some("/dev/hidg*"),
                );
            }
        }
    }

    if !other_gadgets.is_empty() {
        let check_udc = bound_udc.clone().or_else(|| selected_udc.clone());
        if let Some(target_udc) = check_udc {
            let conflicting_gadgets = other_gadgets
                .iter()
                .filter_map(|name| {
                    let udc_file = gadget_root.join(name).join("UDC");
                    let udc = read_trimmed(&udc_file)?;
                    if udc == target_udc {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if conflicting_gadgets.is_empty() {
                push_otg_check(
                    &mut checks,
                    "udc_conflict",
                    true,
                    OtgSelfCheckLevel::Info,
                    "Check UDC binding conflicts",
                    None::<String>,
                    Some(format!("{}/*/UDC", gadget_root.display())),
                );
            } else {
                push_otg_check(
                    &mut checks,
                    "udc_conflict",
                    false,
                    OtgSelfCheckLevel::Error,
                    "Check UDC binding conflicts",
                    Some("Stop other OTG services or switch one-kvm to an idle UDC"),
                    Some(format!("{}/*/UDC", gadget_root.display())),
                );
            }
        }
    }

    let active_udc = bound_udc.clone().or_else(|| selected_udc.clone());
    let mut udc_state = None;
    let mut udc_speed = None;

    if let Some(udc) = active_udc.clone() {
        let state_path = udc_root.join(&udc).join("state");
        match read_trimmed(&state_path) {
            Some(state_name) if state_name.eq_ignore_ascii_case("configured") => {
                udc_state = Some(state_name.clone());
                push_otg_check(
                    &mut checks,
                    "udc_state",
                    true,
                    OtgSelfCheckLevel::Info,
                    "Check UDC connection state",
                    None::<String>,
                    Some(state_path.display().to_string()),
                );
            }
            Some(state_name) => {
                udc_state = Some(state_name.clone());
                push_otg_check(
                    &mut checks,
                    "udc_state",
                    false,
                    OtgSelfCheckLevel::Warn,
                    "Check UDC connection state",
                    Some("Ensure target host is connected and has recognized the USB device"),
                    Some(state_path.display().to_string()),
                );
            }
            None => {
                push_otg_check(
                    &mut checks,
                    "udc_state",
                    false,
                    OtgSelfCheckLevel::Warn,
                    "Check UDC connection state",
                    Some("Ensure UDC name is valid and check kernel permissions"),
                    Some(state_path.display().to_string()),
                );
            }
        }

        let speed_path = udc_root.join(&udc).join("current_speed");
        if let Some(speed) = read_trimmed(&speed_path) {
            udc_speed = Some(speed.clone());
            let is_unknown = speed.eq_ignore_ascii_case("unknown");
            push_otg_check(
                &mut checks,
                "udc_speed",
                !is_unknown,
                if is_unknown {
                    OtgSelfCheckLevel::Warn
                } else {
                    OtgSelfCheckLevel::Info
                },
                "Check UDC current link speed",
                if is_unknown {
                    Some("Device may not be fully enumerated; try reconnecting USB".to_string())
                } else {
                    None
                },
                Some(speed_path.display().to_string()),
            );
        }
    } else {
        push_otg_check(
            &mut checks,
            "udc_state",
            false,
            OtgSelfCheckLevel::Warn,
            "Check UDC connection state",
            Some("Ensure UDC is available and one-kvm gadget is bound first"),
            Some("/sys/class/udc"),
        );
    }

    let error_count = checks
        .iter()
        .filter(|item| item.level == OtgSelfCheckLevel::Error)
        .count();
    let warning_count = checks
        .iter()
        .filter(|item| item.level == OtgSelfCheckLevel::Warn)
        .count();

    OtgSelfCheckResponse {
        overall_ok: error_count == 0,
        error_count,
        warning_count,
        hid_backend: format!("{:?}", config.hid.backend).to_lowercase(),
        selected_udc,
        bound_udc,
        udc_state,
        udc_speed,
        available_udcs,
        other_gadgets,
        checks,
    }
}
