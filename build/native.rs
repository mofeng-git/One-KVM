//! Shared native dependency discovery for the repository's build scripts.
use std::{env, path::PathBuf};

pub fn probe(package: &str, port: &str, static_env: &str) -> Vec<PathBuf> {
    println!("cargo:rerun-if-changed={}", file!());
    println!("cargo:rerun-if-env-changed=ONE_KVM_LIBS_PATH");
    println!("cargo:rerun-if-env-changed={static_env}");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let includes = match target_os.as_str() {
        "windows" => {
            // vcpkg-rs 0.2 does not support VCPKG_INSTALLED_DIR (manifest installs).
            // Resolve the one explicit installation used by build/windows/build.ps1.
            for key in [
                "VCPKG_INSTALLED_DIR",
                "VCPKG_ROOT",
                "VCPKG_DEFAULT_TRIPLET",
                "VCPKGRS_TRIPLET",
            ] {
                println!("cargo:rerun-if-env-changed={key}");
            }
            let installed = env::var_os("VCPKG_INSTALLED_DIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .or_else(|| {
                    env::var_os("VCPKG_ROOT").map(|root| PathBuf::from(root).join("installed"))
                })
                .expect("Set VCPKG_INSTALLED_DIR or VCPKG_ROOT for Windows native dependencies");
            let triplet = env::var("VCPKGRS_TRIPLET")
                .or_else(|_| env::var("VCPKG_DEFAULT_TRIPLET"))
                .unwrap_or_else(|_| {
                    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
                    let arch = match arch.as_str() {
                        "x86_64" => "x64",
                        "x86" => "x86",
                        "aarch64" => "arm64",
                        _ => panic!("Unsupported Windows architecture: {arch}"),
                    };
                    format!("{arch}-windows-static")
                });
            let root = installed.join(triplet);
            let libraries: &[&str] = match port {
                "ffmpeg" => &[
                    "avcodec",
                    "avutil",
                    "avformat",
                    "avdevice",
                    "avfilter",
                    "swresample",
                    "swscale",
                    "vpx",
                    "libx264",
                    "x265-static",
                ],
                "libyuv" => &["yuv"],
                "libjpeg-turbo" => &["turbojpeg"],
                _ => panic!("Unknown vcpkg package: {port}"),
            };
            let libdir = root.join("lib");
            let include = root.join("include");
            assert!(
                include.is_dir(),
                "Missing vcpkg headers: {}",
                include.display()
            );
            println!("cargo:rustc-link-search=native={}", libdir.display());
            for library in libraries {
                assert!(
                    libdir.join(format!("{library}.lib")).is_file(),
                    "Missing vcpkg library {library}.lib in {}",
                    libdir.display()
                );
                println!("cargo:rustc-link-lib=static={library}");
            }
            if port == "ffmpeg" && libdir.join("libmfx.lib").is_file() {
                println!("cargo:rustc-link-lib=static=libmfx");
            }
            println!("cargo:warning={port}: vcpkg {}", root.display());
            vec![include]
        }
        "linux" => {
            // Explicit prefixes must provide metadata for the requested package.
            // Other dependencies are still resolved in the configured target sysroot.
            let mut metadata = None;
            if let Some(root) = env::var_os("ONE_KVM_LIBS_PATH").filter(|root| !root.is_empty()) {
                let directory = PathBuf::from(root).join("lib/pkgconfig");
                assert!(
                    directory.join(format!("{package}.pc")).is_file(),
                    "{package}.pc is required under {} (ONE_KVM_LIBS_PATH)",
                    directory.display()
                );
                let target = env::var("TARGET").unwrap();
                let key = format!("PKG_CONFIG_PATH_{target}");
                metadata = Some(directory.join(format!("{package}.pc")));
                let mut paths = vec![directory];
                if let Some(existing) = env::var_os(&key).or_else(|| env::var_os("PKG_CONFIG_PATH"))
                {
                    paths.extend(env::split_paths(&existing));
                }
                paths.dedup();
                env::set_var(key, env::join_paths(paths).expect("native pkg-config path"));
            }
            let mut config = pkg_config::Config::new();
            if let Ok(value) = env::var(static_env) {
                config.statik(value == "1");
            }
            let package_path = metadata.as_ref().map(|path| path.to_string_lossy());
            let library = config
                .probe(package_path.as_deref().unwrap_or(package))
                .unwrap_or_else(|error| {
                    panic!("target pkg-config package {package} is required: {error}")
                });
            println!(
                "cargo:warning={package}: pkg-config {}, libraries {:?}",
                library.version, library.link_paths
            );
            library.include_paths
        }
        _ => panic!("Unsupported native target OS: {target_os}"),
    };
    println!("cargo:warning={package}: target {target_os}, headers {includes:?}");
    includes
}
