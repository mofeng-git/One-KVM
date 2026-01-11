use cc::Build;
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cpp_dir = manifest_dir.join("cpp");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed={}", cpp_dir.display());
    let mut builder = Build::new();

    build_common(&mut builder);
    ffmpeg::build_ffmpeg(&mut builder);
    builder.static_crt(true).compile("hwcodec");
}

fn build_common(builder: &mut Build) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let common_dir = manifest_dir.join("cpp").join("common");

    bindgen::builder()
        .header(common_dir.join("common.h").to_string_lossy().to_string())
        .header(common_dir.join("callback.h").to_string_lossy().to_string())
        .rustified_enum("*")
        .parse_callbacks(Box::new(CommonCallbacks))
        .generate()
        .unwrap()
        .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("common_ffi.rs"))
        .unwrap();

    // system
    #[cfg(windows)]
    {
        ["d3d11", "dxgi"].map(|lib| println!("cargo:rustc-link-lib={}", lib));
    }

    builder.include(&common_dir);

    // platform
    let platform_path = common_dir.join("platform");
    #[cfg(windows)]
    {
        let win_path = platform_path.join("win");
        builder.include(&win_path);
        builder.file(win_path.join("win.cpp"));
    }
    #[cfg(target_os = "linux")]
    {
        let linux_path = platform_path.join("linux");
        builder.include(&linux_path);
        builder.file(linux_path.join("linux.cpp"));
    }

    // Unsupported platforms
    if target_os != "windows" && target_os != "linux" {
        panic!(
            "Unsupported OS: {}. Only Windows and Linux are supported.",
            target_os
        );
    }

    // tool
    builder.files(["log.cpp", "util.cpp"].map(|f| common_dir.join(f)));
}

#[derive(Debug)]
struct CommonCallbacks;
impl bindgen::callbacks::ParseCallbacks for CommonCallbacks {
    fn add_derives(&self, name: &str) -> Vec<String> {
        let names = vec!["DataFormat", "SurfaceFormat", "API"];
        if names.contains(&name) {
            vec!["Serialize", "Deserialize"]
                .drain(..)
                .map(|s| s.to_string())
                .collect()
        } else {
            vec![]
        }
    }
}

mod ffmpeg {
    use super::*;

    pub fn build_ffmpeg(builder: &mut Build) {
        ffmpeg_ffi();

        // Try VCPKG first, fallback to system FFmpeg via pkg-config
        if let Ok(vcpkg_root) = std::env::var("VCPKG_ROOT") {
            link_vcpkg(builder, vcpkg_root.into());
        } else {
            // Use system FFmpeg via pkg-config
            link_system_ffmpeg(builder);
        }

        link_os();
        build_ffmpeg_ram(builder);
    }

    /// Link system FFmpeg using pkg-config or custom path
    /// Supports both static and dynamic linking based on FFMPEG_STATIC env var
    fn link_system_ffmpeg(builder: &mut Build) {
        use std::process::Command;

        // Check if static linking is requested
        let use_static = std::env::var("FFMPEG_STATIC")
            .map(|v| v == "1")
            .unwrap_or(false);
        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

        // Try custom library path first:
        // 1. Check ONE_KVM_LIBS_PATH environment variable (explicit override)
        // 2. Fall back to architecture-based detection
        let custom_lib_path = if let Ok(path) = std::env::var("ONE_KVM_LIBS_PATH") {
            path
        } else {
            match target_arch.as_str() {
                "x86_64" => "/usr/local",
                "aarch64" => "/usr/aarch64-linux-gnu",
                "arm" => "/usr/arm-linux-gnueabihf",
                _ => "",
            }
            .to_string()
        };

        // Check if our custom FFmpeg exists
        if !custom_lib_path.is_empty() {
            let lib_dir = Path::new(&custom_lib_path).join("lib");
            let include_dir = Path::new(&custom_lib_path).join("include");
            let avcodec_lib = lib_dir.join("libavcodec.a");

            if avcodec_lib.exists() {
                println!("cargo:info=Using custom FFmpeg from {}", custom_lib_path);
                println!("cargo:rustc-link-search=native={}", lib_dir.display());
                builder.include(&include_dir);

                // Static link FFmpeg core libraries
                println!("cargo:rustc-link-lib=static=avcodec");
                println!("cargo:rustc-link-lib=static=avutil");

                // Link hardware acceleration dependencies (dynamic)
                // These vary by architecture
                if target_arch == "x86_64" {
                    // VAAPI for x86_64
                    println!("cargo:rustc-link-lib=va");
                    println!("cargo:rustc-link-lib=va-drm");
                    println!("cargo:rustc-link-lib=va-x11"); // Required for vaGetDisplay
                    println!("cargo:rustc-link-lib=mfx");
                } else {
                    // RKMPP for ARM
                    println!("cargo:rustc-link-lib=rockchip_mpp");
                    let rga_static = lib_dir.join("librga.a");
                    if rga_static.exists() {
                        println!("cargo:rustc-link-lib=static=rga");
                    } else {
                        println!("cargo:rustc-link-lib=rga");
                    }
                }

                // Software codec dependencies (dynamic - GPL)
                println!("cargo:rustc-link-lib=x264");
                println!("cargo:rustc-link-lib=x265");

                // VPX - check if static version exists in our custom path
                let vpx_static = lib_dir.join("libvpx.a");
                if vpx_static.exists() {
                    println!("cargo:rustc-link-lib=static=vpx");
                } else {
                    println!("cargo:rustc-link-lib=vpx");
                }

                return;
            }
        }

        // Fallback to pkg-config
        // Only need libavcodec and libavutil for encoding
        let libs = ["libavcodec", "libavutil"];

        for lib in &libs {
            // Get cflags
            if let Ok(output) = Command::new("pkg-config").args(["--cflags", lib]).output() {
                if output.status.success() {
                    let cflags = String::from_utf8_lossy(&output.stdout);
                    for flag in cflags.split_whitespace() {
                        if flag.starts_with("-I") {
                            builder.include(&flag[2..]);
                        }
                    }
                }
            }

            // Get libs - use --static flag for static linking
            let pkg_config_args = if use_static {
                vec!["--static", "--libs", lib]
            } else {
                vec!["--libs", lib]
            };

            if let Ok(output) = Command::new("pkg-config").args(&pkg_config_args).output() {
                if output.status.success() {
                    let libs_str = String::from_utf8_lossy(&output.stdout);
                    let mut link_paths: Vec<String> = Vec::new();
                    for flag in libs_str.split_whitespace() {
                        if flag.starts_with("-L") {
                            let path = flag[2..].to_string();
                            println!("cargo:rustc-link-search=native={}", path);
                            link_paths.push(path);
                        } else if flag.starts_with("-l") {
                            let lib_name = &flag[2..];
                            if use_static {
                                // For static linking, link FFmpeg libs statically, others dynamically
                                if lib_name.starts_with("av") || lib_name == "swresample" {
                                    println!("cargo:rustc-link-lib=static={}", lib_name);
                                } else if lib_name == "rga"
                                    && link_paths
                                        .iter()
                                        .any(|path| Path::new(path).join("librga.a").exists())
                                {
                                    println!("cargo:rustc-link-lib=static=rga");
                                } else {
                                    // Runtime libraries (va, drm, etc.) must be dynamic
                                    println!("cargo:rustc-link-lib={}", lib_name);
                                }
                            } else {
                                println!("cargo:rustc-link-lib={}", lib_name);
                            }
                        }
                    }
                } else {
                    panic!("pkg-config failed for {}. Install FFmpeg development libraries: sudo apt install libavcodec-dev libavutil-dev", lib);
                }
            } else {
                panic!(
                    "pkg-config not found. Install pkg-config and FFmpeg development libraries."
                );
            }
        }

        if use_static {
            println!("cargo:info=Using system FFmpeg via pkg-config (static linking)");
        } else {
            println!("cargo:info=Using system FFmpeg via pkg-config (dynamic linking)");
        }
    }

    fn link_vcpkg(builder: &mut Build, mut path: PathBuf) -> PathBuf {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
        let mut target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        if target_arch == "x86_64" {
            target_arch = "x64".to_owned();
        } else if target_arch == "x86" {
            target_arch = "x86".to_owned();
        } else if target_arch == "loongarch64" {
            target_arch = "loongarch64".to_owned();
        } else if target_arch == "aarch64" {
            target_arch = "arm64".to_owned();
        } else {
            target_arch = "arm".to_owned();
        }
        let mut target = if target_os == "windows" {
            "x64-windows-static".to_owned()
        } else {
            format!("{}-{}", target_arch, target_os)
        };
        if target_arch == "x86" {
            target = target.replace("x64", "x86");
        }
        println!("cargo:info={}", target);
        path.push("installed");
        path.push(target);

        println!(
            "{}",
            format!(
                "cargo:rustc-link-search=native={}",
                path.join("lib").to_str().unwrap()
            )
        );
        {
            // Only need avcodec and avutil for encoding
            let mut static_libs = vec!["avcodec", "avutil"];
            if target_os == "windows" {
                static_libs.push("libmfx");
            }
            static_libs
                .iter()
                .map(|lib| println!("cargo:rustc-link-lib=static={}", lib))
                .count();
        }

        let include = path.join("include");
        println!("{}", format!("cargo:include={}", include.to_str().unwrap()));
        builder.include(&include);
        include
    }

    fn link_os() {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

        let dyn_libs: Vec<&str> = if target_os == "windows" {
            ["User32", "bcrypt", "ole32", "advapi32"].to_vec()
        } else if target_os == "linux" {
            // Base libraries for all Linux platforms
            let mut v = vec!["drm", "stdc++"];

            // x86_64: needs X11 for VAAPI and zlib
            if target_arch == "x86_64" {
                v.push("X11");
                v.push("z");
            }
            // ARM (aarch64, arm): no X11 needed, uses RKMPP/V4L2
            v
        } else {
            panic!(
                "Unsupported OS: {}. Only Windows and Linux are supported.",
                target_os
            );
        };

        for lib in dyn_libs.iter() {
            println!("cargo:rustc-link-lib={}", lib);
        }
    }

    fn ffmpeg_ffi() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ffmpeg_ram_dir = manifest_dir.join("cpp").join("common");
        let ffi_header_path = ffmpeg_ram_dir.join("ffmpeg_ffi.h");
        println!("cargo:rerun-if-changed={}", ffi_header_path.display());
        let ffi_header = ffi_header_path.to_string_lossy().to_string();
        bindgen::builder()
            .header(ffi_header)
            .rustified_enum("*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("ffmpeg_ffi.rs"))
            .unwrap();
    }

    fn build_ffmpeg_ram(builder: &mut Build) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ffmpeg_ram_dir = manifest_dir.join("cpp").join("ffmpeg_ram");
        let ffi_header = ffmpeg_ram_dir
            .join("ffmpeg_ram_ffi.h")
            .to_string_lossy()
            .to_string();
        bindgen::builder()
            .header(ffi_header)
            .rustified_enum("*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("ffmpeg_ram_ffi.rs"))
            .unwrap();

        builder.file(ffmpeg_ram_dir.join("ffmpeg_ram_encode.cpp"));

        // RKMPP decode only exists on ARM builds where FFmpeg is compiled with RKMPP support.
        // Avoid compiling this file on x86/x64 where `AV_HWDEVICE_TYPE_RKMPP` doesn't exist.
        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        let enable_rkmpp = matches!(target_arch.as_str(), "aarch64" | "arm")
            || std::env::var_os("CARGO_FEATURE_RKMPP").is_some();
        if enable_rkmpp {
            builder.file(ffmpeg_ram_dir.join("ffmpeg_ram_decode.cpp"));
        } else {
            println!(
                "cargo:info=Skipping ffmpeg_ram_decode.cpp (RKMPP) for arch {}",
                target_arch
            );
        }
    }
}
