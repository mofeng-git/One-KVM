use std::env::{self, VarError};
use std::path::PathBuf;

include!("bindgen.rs");

/// Environment variable that can be set to point to the directory containing the `videodev2.h`
/// file to use to generate the bindings.
const V4L2R_VIDEODEV_ENV: &str = "V4L2R_VIDEODEV2_H_PATH";

/// Default header file to parse if the `V4L2R_VIDEODEV2_H_PATH` environment variable is not set.
const DEFAULT_VIDEODEV2_H_PATH: &str = "/usr/include/linux";

/// Wrapper file to use as input of bindgen.
const WRAPPER_H: &str = "v4l2r_wrapper.h";

// Fix for https://github.com/rust-lang/rust-bindgen/issues/753
const FIX753_H: &str = "fix753.h";

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    let is_android = target.contains("android");

    let default_videodev2_h_path = if is_android {
        android_sysroot().join("usr/include").display().to_string()
    } else {
        DEFAULT_VIDEODEV2_H_PATH.to_string()
    };

    let videodev2_h_path = env::var(V4L2R_VIDEODEV_ENV)
        .or_else(|e| {
            if let VarError::NotPresent = e {
                Ok(default_videodev2_h_path.clone())
            } else {
                Err(e)
            }
        })
        .expect("invalid `V4L2R_VIDEODEV2_H_PATH` environment variable");

    let videodev2_h = PathBuf::from(videodev2_h_path.clone()).join(if is_android {
        "linux/videodev2.h"
    } else {
        "videodev2.h"
    });

    println!("cargo::rerun-if-env-changed={}", V4L2R_VIDEODEV_ENV);
    println!("cargo::rerun-if-env-changed=ANDROID_NDK_HOME");
    println!("cargo::rerun-if-env-changed=ANDROID_NDK_ROOT");
    println!("cargo::rerun-if-env-changed=NDK_HOME");
    println!("cargo::rerun-if-env-changed=ANDROID_HOME");
    println!("cargo::rerun-if-env-changed=ANDROID_SDK_ROOT");
    println!("cargo::rerun-if-env-changed=CARGO_NDK_PLATFORM");
    println!("cargo::rerun-if-changed={}", videodev2_h.display());
    println!("cargo::rerun-if-changed={}", FIX753_H);
    println!("cargo::rerun-if-changed={}", WRAPPER_H);

    let mut clang_args = vec![
        format!("-I{videodev2_h_path}"),
        #[cfg(all(feature = "arch64", not(feature = "arch32")))]
        "--target=x86_64-linux-gnu".into(),
        #[cfg(all(feature = "arch32", not(feature = "arch64")))]
        "--target=i686-linux-gnu".into(),
    ];

    if is_android {
        clang_args.extend(android_clang_args(&target));
    }

    let bindings = v4l2r_bindgen_builder(bindgen::Builder::default())
        .header(WRAPPER_H)
        .clang_args(clang_args)
        .generate()
        .expect("unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").expect("`OUT_DIR` is not set"));
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn android_clang_args(target: &str) -> Vec<String> {
    let ndk = android_ndk_home();
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
    let clang_target = android_clang_target(target);

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
        other => panic!("unsupported Android target for v4l2r bindgen: {other}"),
    }
}

fn android_sysroot() -> PathBuf {
    android_ndk_home()
        .join("toolchains/llvm/prebuilt")
        .join(host_tag())
        .join("sysroot")
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
        "v4l2r Android bindgen requires ANDROID_NDK_HOME, ANDROID_NDK_ROOT, NDK_HOME, \
         or ANDROID_HOME/ANDROID_SDK_ROOT with an ndk directory"
    );
}

fn newest_child_dir(path: &PathBuf) -> Option<PathBuf> {
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

fn clang_version(toolchain: &PathBuf) -> String {
    let clang_dir = toolchain.join("lib/clang");
    let mut entries = std::fs::read_dir(&clang_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", clang_dir.display()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    entries.sort();
    entries
        .pop()
        .unwrap_or_else(|| panic!("no clang resource directory in {}", clang_dir.display()))
}
