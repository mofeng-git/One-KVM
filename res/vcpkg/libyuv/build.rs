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
    let mut builder = bindgen::builder().header(ffi_header.to_string_lossy().to_string());

    if env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("android") {
        println!("cargo:rerun-if-env-changed=ANDROID_NDK_HOME");
        println!("cargo:rerun-if-env-changed=ANDROID_NDK_ROOT");
        println!("cargo:rerun-if-env-changed=NDK_HOME");
        println!("cargo:rerun-if-env-changed=ANDROID_HOME");
        println!("cargo:rerun-if-env-changed=ANDROID_SDK_ROOT");
        println!("cargo:rerun-if-env-changed=CARGO_NDK_PLATFORM");
        builder = builder.clang_args(android_clang_args());
    }

    builder
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
        .allowlist_function("I444ToI420")
        // NV12/NV21 conversions
        .allowlist_function("NV12ToI420")
        .allowlist_function("NV21ToI420")
        .allowlist_function("NV21ToNV12")
        .allowlist_function("NV12Copy")
        .allowlist_function("SplitUVPlane")
        // ARGB/BGRA conversions
        .allowlist_function("ARGBToI420")
        .allowlist_function("ARGBToNV12")
        .allowlist_function("ABGRToI420")
        .allowlist_function("ABGRToNV12")
        .allowlist_function("ARGBToABGR")
        .allowlist_function("ABGRToARGB")
        // RGB24/BGR24 conversions
        .allowlist_function("RGB24ToI420")
        .allowlist_function("RGB24ToNV12")
        .allowlist_function("RAWToI420")
        .allowlist_function("RGB24ToARGB")
        .allowlist_function("RAWToARGB")
        // YUV to RGB conversions
        .allowlist_function("I420ToRGB24")
        .allowlist_function("I420ToARGB")
        .allowlist_function("H444ToARGB")
        .allowlist_function("NV12ToRGB24")
        .allowlist_function("NV12ToARGB")
        .allowlist_function("YUY2ToARGB")
        .allowlist_function("UYVYToARGB")
        .allowlist_function("ARGBToRGB24")
        .allowlist_function("ARGBToRAW")
        // MJPEG decoding
        .allowlist_function("MJPGToNV12")
        .allowlist_function("MJPGSize")
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
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if target_os == "android" {
        if link_android_libyuv() {
            return;
        }
        if let Some(vcpkg_installed) = vcpkg_installed_root() {
            if link_vcpkg(vcpkg_installed) {
                return;
            }
        }

        panic!(
            "Android libyuv not found!\n\
             \n\
             Build it with scripts/build-android-libyuv.sh and set:\n\
             export ONE_KVM_ANDROID_LIBYUV_ROOT=/path/to/android-libyuv\n\
             \n\
             Expected layout:\n\
             $ONE_KVM_ANDROID_LIBYUV_ROOT/<abi>/include\n\
             $ONE_KVM_ANDROID_LIBYUV_ROOT/<abi>/lib/libyuv.a"
        );
    }

    // Try vcpkg first
    if let Some(vcpkg_installed) = vcpkg_installed_root() {
        if link_vcpkg(vcpkg_installed) {
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

fn link_android_libyuv() -> bool {
    println!("cargo:rerun-if-env-changed=ONE_KVM_ANDROID_LIBYUV_ROOT");
    println!("cargo:rerun-if-env-changed=ONE_KVM_ANDROID_LIBYUV_STATIC");

    let root = match env::var("ONE_KVM_ANDROID_LIBYUV_ROOT")
        .ok()
        .filter(|path| !path.trim().is_empty())
    {
        Some(path) => PathBuf::from(path),
        None => return false,
    };

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let abi = android_abi(&target_arch);
    let abi_root = root.join(abi);
    let lib_dir = if abi_root.join("lib").exists() {
        abi_root.join("lib")
    } else {
        root.join("lib")
    };
    let include_dir = if abi_root.join("include").exists() {
        abi_root.join("include")
    } else {
        root.join("include")
    };

    let static_lib = lib_dir.join("libyuv.a");
    let shared_lib = lib_dir.join("libyuv.so");
    let use_static = env::var("ONE_KVM_ANDROID_LIBYUV_STATIC")
        .or_else(|_| env::var("LIBYUV_STATIC"))
        .map(|value| value != "0")
        .unwrap_or(true);

    if use_static && static_lib.exists() {
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=static=yuv");
        link_android_libjpeg(&root, abi);
        println!("cargo:rustc-link-lib=c++_shared");
        println!(
            "cargo:info=Using Android libyuv from {} (static linking)",
            root.display()
        );
        return true;
    }

    if shared_lib.exists() {
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=yuv");
        println!("cargo:rustc-link-lib=c++_shared");
        println!(
            "cargo:info=Using Android libyuv from {} (dynamic linking)",
            root.display()
        );
        return true;
    }

    println!(
        "cargo:warning=Android libyuv not found under {} for ABI {} (checked {}, {})",
        root.display(),
        abi,
        static_lib.display(),
        shared_lib.display()
    );
    if !include_dir.exists() {
        println!(
            "cargo:warning=Android libyuv include directory not found: {}",
            include_dir.display()
        );
    }
    false
}

fn link_android_libjpeg(libyuv_root: &Path, abi: &str) {
    println!("cargo:rerun-if-env-changed=ONE_KVM_ANDROID_TURBOJPEG_ROOT");

    let mut roots = Vec::new();
    if let Ok(root) = env::var("ONE_KVM_ANDROID_TURBOJPEG_ROOT") {
        if !root.trim().is_empty() {
            roots.push(PathBuf::from(root));
        }
    }
    roots.push(libyuv_root.with_file_name("android-turbojpeg"));

    for root in roots {
        let abi_lib_dir = root.join(abi).join("lib");
        let lib_dir = if abi_lib_dir.exists() {
            abi_lib_dir
        } else {
            root.join("lib")
        };
        let jpeg_lib = lib_dir.join("libjpeg.a");
        if jpeg_lib.exists() {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=static=jpeg");
            println!(
                "cargo:info=Using Android libjpeg for libyuv MJPEG from {}",
                root.display()
            );
            return;
        }
    }

    println!("cargo:warning=Android libjpeg.a not found; libyuv MJPEG symbols may fail to link");
}

fn android_abi(target_arch: &str) -> &'static str {
    match target_arch {
        "aarch64" => "arm64-v8a",
        "arm" => "armeabi-v7a",
        "x86" => "x86",
        "x86_64" => "x86_64",
        _ => "unknown",
    }
}

fn android_clang_args() -> Vec<String> {
    let ndk = android_ndk_home();
    let target = env::var("TARGET").unwrap_or_default();
    let toolchain = ndk.join("toolchains/llvm/prebuilt").join(host_tag());
    let sysroot = toolchain.join("sysroot");
    let clang_include = toolchain
        .join("lib/clang")
        .join(clang_version(&toolchain))
        .join("include");
    let api = env::var("CARGO_NDK_PLATFORM")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(21);
    let clang_target = android_clang_target(&target);

    vec![
        format!("--target={clang_target}"),
        format!("--sysroot={}", sysroot.display()),
        format!("-D__ANDROID_API__={api}"),
        format!("-isystem{}", clang_include.display()),
        format!("-isystem{}", sysroot.join("usr/include").display()),
        format!(
            "-isystem{}",
            sysroot.join("usr/include").join(clang_target).display()
        ),
    ]
}

fn android_clang_target(target: &str) -> &'static str {
    match target {
        "aarch64-linux-android" => "aarch64-linux-android",
        "armv7-linux-androideabi" => "armv7a-linux-androideabi",
        "i686-linux-android" => "i686-linux-android",
        "x86_64-linux-android" => "x86_64-linux-android",
        other => panic!("unsupported Android target for libyuv bindgen: {other}"),
    }
}

fn android_ndk_home() -> PathBuf {
    for key in ["ANDROID_NDK_HOME", "ANDROID_NDK_ROOT", "NDK_HOME"] {
        if let Ok(value) = env::var(key) {
            return PathBuf::from(value);
        }
    }

    for key in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Ok(value) = env::var(key) {
            let ndk_dir = PathBuf::from(value).join("ndk");
            if let Some(newest) = newest_child_dir(&ndk_dir) {
                return newest;
            }
        }
    }

    panic!(
        "libyuv Android bindgen requires ANDROID_NDK_HOME, ANDROID_NDK_ROOT, NDK_HOME, \
         or ANDROID_HOME/ANDROID_SDK_ROOT with an ndk directory"
    );
}

fn newest_child_dir(path: &Path) -> Option<PathBuf> {
    let mut entries = std::fs::read_dir(path)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    entries.sort();
    entries.pop()
}

fn host_tag() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux-x86_64"
    } else if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else {
        panic!("unsupported host OS for Android NDK");
    }
}

fn clang_version(toolchain: &Path) -> String {
    let clang_dir = toolchain.join("lib/clang");
    let mut entries = std::fs::read_dir(&clang_dir)
        .unwrap_or_else(|_| panic!("missing NDK clang directory: {}", clang_dir.display()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    entries.sort();
    entries
        .pop()
        .unwrap_or_else(|| panic!("no clang versions found under: {}", clang_dir.display()))
}

fn vcpkg_installed_root() -> Option<PathBuf> {
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLED_DIR");
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");

    if let Ok(path) = env::var("VCPKG_INSTALLED_DIR") {
        if !path.trim().is_empty() {
            return Some(PathBuf::from(path));
        }
    }

    env::var("VCPKG_ROOT")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .map(|path| PathBuf::from(path).join("installed"))
}

fn link_vcpkg(mut path: PathBuf) -> bool {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    let triplet = match (target_os.as_str(), target_arch.as_str()) {
        ("linux", "x86_64") => "x64-linux",
        ("linux", "aarch64") => "arm64-linux",
        ("linux", "arm") => "arm-linux",
        ("android", "x86_64") => "x64-android",
        ("android", "x86") => "x86-android",
        ("android", "aarch64") => "arm64-android",
        ("android", "arm") => "arm-neon-android",
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
        link_libjpeg_for_static_libyuv(&[lib_path.clone()], &target_os);
        if target_os == "linux" {
            println!("cargo:rustc-link-lib=stdc++");
        } else if target_os == "android" {
            println!("cargo:rustc-link-lib=c++_shared");
        }
        println!("cargo:info=Using libyuv from vcpkg (static linking)");
    } else {
        // Dynamic linking (default for development)
        println!("cargo:rustc-link-lib=yuv");
        if target_os == "linux" {
            println!("cargo:rustc-link-lib=stdc++");
        } else if target_os == "android" {
            println!("cargo:rustc-link-lib=c++_shared");
        }
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
            link_libjpeg_for_static_libyuv(&[lib_path.to_path_buf()], "linux");
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

fn link_libjpeg_for_static_libyuv(preferred_lib_dirs: &[PathBuf], target_os: &str) {
    if target_os != "linux" {
        return;
    }

    println!("cargo:rerun-if-env-changed=ONE_KVM_LIBJPEG_DIR");

    let mut lib_dirs = Vec::new();
    if let Ok(path) = env::var("ONE_KVM_LIBJPEG_DIR") {
        if !path.trim().is_empty() {
            lib_dirs.push(PathBuf::from(path));
        }
    }
    lib_dirs.extend(preferred_lib_dirs.iter().cloned());
    lib_dirs.extend(
        [
            "/usr/local/lib",
            "/usr/local/lib64",
            "/usr/lib",
            "/usr/lib64",
            "/usr/lib/x86_64-linux-gnu",
            "/usr/lib/aarch64-linux-gnu",
            "/usr/lib/arm-linux-gnueabihf",
            "/usr/aarch64-linux-gnu/lib",
            "/usr/arm-linux-gnueabihf/lib",
        ]
        .iter()
        .map(PathBuf::from),
    );

    for lib_dir in dedupe_paths(lib_dirs) {
        if lib_dir.join("libjpeg.a").exists() {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=static=jpeg");
            println!(
                "cargo:info=Using libjpeg for static libyuv MJPEG from {}",
                lib_dir.display()
            );
            return;
        }
    }

    println!("cargo:warning=libjpeg.a not found; static libyuv built with MJPEG may fail to link");
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    for path in paths {
        if !deduped.iter().any(|existing| existing == &path) {
            deduped.push(path);
        }
    }
    deduped
}
