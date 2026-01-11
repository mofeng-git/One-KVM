use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cpp_dir = manifest_dir.join("cpp");

    println!("cargo:rerun-if-changed=cpp/yuv_ffi.h");

    // Generate FFI bindings
    generate_bindings(&cpp_dir);

    // Link libyuv library
    link_libyuv();
}

fn generate_bindings(cpp_dir: &Path) {
    let ffi_header = cpp_dir.join("yuv_ffi.h");

    bindgen::builder()
        .header(ffi_header.to_string_lossy().to_string())
        // YUYV conversions
        .allowlist_function("YUY2ToI420")
        .allowlist_function("YUY2ToNV12")
        // UYVY conversions
        .allowlist_function("UYVYToI420")
        .allowlist_function("UYVYToNV12")
        // I420 conversions
        .allowlist_function("I420ToNV12")
        .allowlist_function("I420ToNV21")
        .allowlist_function("I420Copy")
        // I422 conversions
        .allowlist_function("I422ToI420")
        // NV12/NV21 conversions
        .allowlist_function("NV12ToI420")
        .allowlist_function("NV21ToI420")
        .allowlist_function("NV12Copy")
        // ARGB/BGRA conversions
        .allowlist_function("ARGBToI420")
        .allowlist_function("ARGBToNV12")
        .allowlist_function("ABGRToI420")
        .allowlist_function("ABGRToNV12")
        .allowlist_function("ARGBToABGR")
        .allowlist_function("ABGRToARGB")
        // RGB24/BGR24 conversions
        .allowlist_function("RGB24ToI420")
        .allowlist_function("RAWToI420")
        .allowlist_function("RGB24ToARGB")
        .allowlist_function("RAWToARGB")
        // YUV to RGB conversions
        .allowlist_function("I420ToRGB24")
        .allowlist_function("I420ToARGB")
        .allowlist_function("NV12ToRGB24")
        .allowlist_function("NV12ToARGB")
        .allowlist_function("YUY2ToARGB")
        .allowlist_function("UYVYToARGB")
        .allowlist_function("ARGBToRGB24")
        .allowlist_function("ARGBToRAW")
        // Scaling
        .allowlist_function("I420Scale")
        .allowlist_function("NV12Scale")
        .allowlist_function("ARGBScale")
        // Rotation
        .allowlist_function("I420Rotate")
        .allowlist_function("NV12ToI420Rotate")
        // Enums
        .allowlist_type("FilterMode")
        .allowlist_type("RotationMode")
        .rustified_enum("FilterMode")
        .rustified_enum("RotationMode")
        .generate()
        .expect("Failed to generate libyuv bindings")
        .write_to_file(Path::new(&env::var_os("OUT_DIR").unwrap()).join("yuv_ffi.rs"))
        .expect("Failed to write yuv_ffi.rs");
}

fn link_libyuv() {
    // Try vcpkg first
    if let Ok(vcpkg_root) = env::var("VCPKG_ROOT") {
        if link_vcpkg(vcpkg_root.into()) {
            return;
        }
    }

    // Try pkg-config
    if link_pkg_config() {
        return;
    }

    // Try system library directly
    if link_system() {
        return;
    }

    panic!(
        "libyuv not found!\n\
         \n\
         Install via one of:\n\
         - vcpkg: vcpkg install libyuv && export VCPKG_ROOT=/path/to/vcpkg\n\
         - apt (Debian/Ubuntu): sudo apt install libyuv-dev\n\
         - dnf (Fedora): sudo dnf install libyuv-devel\n\
         - pacman (Arch): sudo pacman -S libyuv\n"
    );
}

fn link_vcpkg(mut path: PathBuf) -> bool {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    let triplet = match (target_os.as_str(), target_arch.as_str()) {
        ("linux", "x86_64") => "x64-linux",
        ("linux", "aarch64") => "arm64-linux",
        ("linux", "arm") => "arm-linux",
        ("windows", "x86_64") => "x64-windows-static",
        ("windows", "x86") => "x86-windows-static",
        ("macos", "x86_64") => "x64-osx",
        ("macos", "aarch64") => "arm64-osx",
        _ => {
            println!(
                "cargo:warning=Unsupported vcpkg target: {}-{}",
                target_os, target_arch
            );
            return false;
        }
    };

    path.push("installed");
    path.push(triplet);

    let include_path = path.join("include");
    let lib_path = path.join("lib");

    if !lib_path.exists() {
        println!(
            "cargo:warning=vcpkg libyuv not found at: {}",
            lib_path.display()
        );
        return false;
    }

    println!("cargo:rustc-link-search=native={}", lib_path.display());

    // Check if static linking is requested via environment variable
    let use_static = env::var("LIBYUV_STATIC").map(|v| v == "1").unwrap_or(false);

    let static_lib = lib_path.join("libyuv.a");

    if use_static && static_lib.exists() {
        // Static linking (for deb packaging)
        println!("cargo:rustc-link-lib=static=yuv");
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:info=Using libyuv from vcpkg (static linking)");
    } else {
        // Dynamic linking (default for development)
        println!("cargo:rustc-link-lib=yuv");
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:info=Using libyuv from vcpkg (dynamic linking)");
    }

    println!(
        "cargo:info=Using libyuv from vcpkg: {}",
        include_path.display()
    );
    true
}

fn link_pkg_config() -> bool {
    let output = match Command::new("pkg-config")
        .args(["--libs", "--cflags", "libyuv"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };

    if !output.status.success() {
        return false;
    }

    let flags = String::from_utf8_lossy(&output.stdout);
    for flag in flags.split_whitespace() {
        if flag.starts_with("-L") {
            println!("cargo:rustc-link-search=native={}", &flag[2..]);
        } else if flag.starts_with("-l") {
            println!("cargo:rustc-link-lib={}", &flag[2..]);
        }
    }

    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=stdc++");

    println!("cargo:info=Using libyuv from pkg-config (dynamic linking)");
    true
}

fn link_system() -> bool {
    // Check if static linking is requested
    let use_static = env::var("LIBYUV_STATIC").map(|v| v == "1").unwrap_or(false);
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // Build custom library paths based on target architecture:
    // 1. Check ONE_KVM_LIBS_PATH environment variable (explicit override)
    // 2. Fall back to architecture-based detection
    let custom_lib_path = if let Ok(path) = env::var("ONE_KVM_LIBS_PATH") {
        format!("{}/lib", path)
    } else {
        match target_arch.as_str() {
            "x86_64" => "/usr/local/lib",
            "aarch64" => "/usr/aarch64-linux-gnu/lib",
            "arm" => "/usr/arm-linux-gnueabihf/lib",
            _ => "",
        }
        .to_string()
    };

    // Try common system library paths (custom paths first)
    let mut lib_paths: Vec<String> = Vec::new();

    // Add custom build path first (highest priority)
    if !custom_lib_path.is_empty() {
        lib_paths.push(custom_lib_path);
    }

    // Then standard paths
    lib_paths.extend(
        [
            "/usr/local/lib", // Custom builds
            "/usr/local/lib64",
            "/usr/lib",
            "/usr/lib64",
            "/usr/lib/x86_64-linux-gnu",    // Debian/Ubuntu x86_64
            "/usr/lib/aarch64-linux-gnu",   // Debian/Ubuntu ARM64
            "/usr/lib/arm-linux-gnueabihf", // Debian/Ubuntu ARMv7
        ]
        .iter()
        .map(|s| s.to_string()),
    );

    for path in &lib_paths {
        let lib_path = Path::new(path);
        let libyuv_static = lib_path.join("libyuv.a");
        let libyuv_so = lib_path.join("libyuv.so");

        // Prefer static library if LIBYUV_STATIC=1
        if use_static && libyuv_static.exists() {
            println!("cargo:rustc-link-search=native={}", path);
            println!("cargo:rustc-link-lib=static=yuv");
            println!("cargo:rustc-link-lib=stdc++");
            println!(
                "cargo:info=Using system libyuv from {} (static linking)",
                path
            );
            return true;
        }

        // Fall back to dynamic linking
        if libyuv_so.exists() {
            println!("cargo:rustc-link-search=native={}", path);
            println!("cargo:rustc-link-lib=yuv");

            #[cfg(target_os = "linux")]
            println!("cargo:rustc-link-lib=stdc++");

            println!(
                "cargo:info=Using system libyuv from {} (dynamic linking)",
                path
            );
            return true;
        }
    }

    false
}
