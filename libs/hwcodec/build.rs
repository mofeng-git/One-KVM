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
        panic!("Unsupported OS: {}. Only Windows and Linux are supported.", target_os);
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

    /// Link system FFmpeg using pkg-config (for Linux development)
    fn link_system_ffmpeg(builder: &mut Build) {
        use std::process::Command;

        let libs = ["libavcodec", "libavutil", "libavformat", "libswscale"];

        for lib in &libs {
            // Get cflags
            if let Ok(output) = Command::new("pkg-config")
                .args(["--cflags", lib])
                .output()
            {
                if output.status.success() {
                    let cflags = String::from_utf8_lossy(&output.stdout);
                    for flag in cflags.split_whitespace() {
                        if flag.starts_with("-I") {
                            builder.include(&flag[2..]);
                        }
                    }
                }
            }

            // Get libs - always use dynamic linking on Linux
            if let Ok(output) = Command::new("pkg-config")
                .args(["--libs", lib])
                .output()
            {
                if output.status.success() {
                    let libs_str = String::from_utf8_lossy(&output.stdout);
                    for flag in libs_str.split_whitespace() {
                        if flag.starts_with("-L") {
                            println!("cargo:rustc-link-search=native={}", &flag[2..]);
                        } else if flag.starts_with("-l") {
                            println!("cargo:rustc-link-lib={}", &flag[2..]);
                        }
                    }
                } else {
                    panic!("pkg-config failed for {}. Install FFmpeg development libraries: sudo apt install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev", lib);
                }
            } else {
                panic!("pkg-config not found. Install pkg-config and FFmpeg development libraries.");
            }
        }

        println!("cargo:info=Using system FFmpeg via pkg-config (dynamic linking)");
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
            let mut static_libs = vec!["avcodec", "avutil", "avformat"];
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
            // Note: VA-API libraries (va, va-drm, va-x11) must remain dynamic
            // as they load drivers at runtime. Same for libmfx.
            // All dependencies use dynamic linking on Linux.
            let mut v = vec!["drm", "X11", "stdc++"];

            if target_arch == "x86_64" {
                v.push("z");
            }
            v
        } else {
            panic!("Unsupported OS: {}. Only Windows and Linux are supported.", target_os);
        };

        for lib in dyn_libs.iter() {
            println!("cargo:rustc-link-lib={}", lib);
        }
    }

    fn ffmpeg_ffi() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ffmpeg_ram_dir = manifest_dir.join("cpp").join("common");
        let ffi_header = ffmpeg_ram_dir
            .join("ffmpeg_ffi.h")
            .to_string_lossy()
            .to_string();
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

        builder.files(
            ["ffmpeg_ram_encode.cpp", "ffmpeg_ram_decode.cpp"].map(|f| ffmpeg_ram_dir.join(f)),
        );
    }
}
