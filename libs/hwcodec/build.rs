#[path = "../../build/native.rs"]
mod native;

use cc::Build;
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    // Read the current source path when running, since build artifacts may move.
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cpp_dir = manifest_dir.join("cpp");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed={}", cpp_dir.display());
    let mut builder = Build::new();

    build_common(&mut builder);
    ffmpeg::build_ffmpeg(&mut builder);
    builder.static_crt(true).compile("hwcodec");
}

fn build_common(builder: &mut Build) {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let common_dir = manifest_dir.join("cpp").join("common");

    let bindings = bindgen::builder()
        .header(common_dir.join("common.h").to_string_lossy().to_string())
        .header(common_dir.join("callback.h").to_string_lossy().to_string())
        .rustified_enum(".*")
        .parse_callbacks(Box::new(CommonCallbacks));
    bindings
        .generate()
        .unwrap()
        .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("common_ffi.rs"))
        .unwrap();

    // system
    #[cfg(windows)]
    {
        for lib in ["d3d11", "dxgi"] {
            println!("cargo:rustc-link-lib={}", lib);
        }
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
    fn add_derives(&self, info: &bindgen::callbacks::DeriveInfo<'_>) -> Vec<String> {
        let names = vec!["DataFormat", "SurfaceFormat", "API"];
        if names.contains(&info.name) {
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
        let mut includes = Vec::new();
        if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            includes.extend(native::probe("libavcodec", "ffmpeg", "FFMPEG_STATIC"));
        } else {
            for package in ["libavcodec", "libavutil"] {
                includes.extend(native::probe(package, "ffmpeg", "FFMPEG_STATIC"));
            }
        }
        if env::var_os("CARGO_FEATURE_RKMPP").is_some() {
            includes.extend(native::probe("librga", "librga", "FFMPEG_STATIC"));
        }
        for path in &includes {
            builder.include(path);
        }
        ffmpeg_ffi(&includes);
        link_os();
        build_ffmpeg_ram(builder, &includes);
        build_ffmpeg_hw(builder, &includes);
        build_ffmpeg_capture(builder, &includes);
    }

    fn link_os() {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

        let dyn_libs: Vec<&str> = if target_os == "windows" {
            [
                "User32", "bcrypt", "ole32", "advapi32", "mfuuid", "strmiids",
            ]
            .to_vec()
        } else if target_os == "linux" {
            // Base libraries for all Linux platforms
            let mut v = vec!["drm", "stdc++"];

            // FFmpeg owns its display backend dependencies. The hwcodec bridge
            // does not call Xlib, and headless Linux sysroots need no X11.
            if target_arch == "x86_64" {
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

    fn ffmpeg_ffi(includes: &[PathBuf]) {
        let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let ffmpeg_ram_dir = manifest_dir.join("cpp").join("common");
        let ffi_header_path = ffmpeg_ram_dir.join("ffmpeg_ffi.h");
        println!("cargo:rerun-if-changed={}", ffi_header_path.display());
        let ffi_header = ffi_header_path.to_string_lossy().to_string();
        let bindings = bindgen::builder()
            .header(ffi_header)
            .clang_args(includes.iter().map(|path| format!("-I{}", path.display())))
            .rustified_enum(".*");
        bindings
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("ffmpeg_ffi.rs"))
            .unwrap();
    }

    fn build_ffmpeg_ram(builder: &mut Build, includes: &[PathBuf]) {
        let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let ffmpeg_ram_dir = manifest_dir.join("cpp").join("ffmpeg_ram");
        let ffi_header = ffmpeg_ram_dir
            .join("ffmpeg_ram_ffi.h")
            .to_string_lossy()
            .to_string();
        let bindings = bindgen::builder()
            .header(ffi_header)
            .clang_args(includes.iter().map(|path| format!("-I{}", path.display())))
            .rustified_enum(".*");
        bindings
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("ffmpeg_ram_ffi.rs"))
            .unwrap();

        builder.file(ffmpeg_ram_dir.join("ffmpeg_ram_encode.cpp"));

        // RKMPP decode only exists on ARM builds where FFmpeg is compiled with RKMPP support.
        // Avoid compiling this file on x86/x64 where `AV_HWDEVICE_TYPE_RKMPP` doesn't exist.
        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        let enable_rkmpp = std::env::var_os("CARGO_FEATURE_RKMPP").is_some();
        if enable_rkmpp {
            builder.file(ffmpeg_ram_dir.join("ffmpeg_ram_decode.cpp"));
            builder.define("ONE_KVM_FFMPEG_RKMPP", None);
        } else {
            println!(
                "cargo:info=Skipping ffmpeg_ram_decode.cpp (RKMPP) for arch {}",
                target_arch
            );
        }
    }

    fn build_ffmpeg_capture(builder: &mut Build, includes: &[PathBuf]) {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if target_os != "windows" {
            return;
        }

        let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let capture_header = manifest_dir
            .join("cpp")
            .join("ffmpeg_capture_ffi.h")
            .to_string_lossy()
            .to_string();
        bindgen::builder()
            .clang_args(includes.iter().map(|path| format!("-I{}", path.display())))
            .header(capture_header)
            .rustified_enum(".*")
            .generate()
            .unwrap()
            .write_to_file(
                Path::new(&env::var_os("OUT_DIR").unwrap()).join("ffmpeg_capture_ffi.rs"),
            )
            .unwrap();

        builder.file(manifest_dir.join("cpp").join("ffmpeg_capture.cpp"));
        println!("cargo:rustc-link-lib=strmiids");
        println!("cargo:rustc-link-lib=oleaut32");
        println!("cargo:rustc-link-lib=quartz");
    }

    fn build_ffmpeg_hw(builder: &mut Build, includes: &[PathBuf]) {
        let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let ffmpeg_hw_dir = manifest_dir.join("cpp").join("ffmpeg_hw");
        let ffi_header = ffmpeg_hw_dir
            .join("ffmpeg_hw_ffi.h")
            .to_string_lossy()
            .to_string();
        bindgen::builder()
            .clang_args(includes.iter().map(|path| format!("-I{}", path.display())))
            .header(ffi_header)
            .rustified_enum(".*")
            .generate()
            .unwrap()
            .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("ffmpeg_hw_ffi.rs"))
            .unwrap();

        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        let enable_rkmpp = std::env::var_os("CARGO_FEATURE_RKMPP").is_some();
        if enable_rkmpp {
            builder.file(ffmpeg_hw_dir.join("ffmpeg_hw_mjpeg_h26x.cpp"));
            builder.file(ffmpeg_hw_dir.join("rkmpp_dmabuf.cpp"));
        } else {
            println!(
                "cargo:info=Skipping ffmpeg_hw_mjpeg_h26x.cpp (RKMPP) for arch {}",
                target_arch
            );
        }
    }
}
