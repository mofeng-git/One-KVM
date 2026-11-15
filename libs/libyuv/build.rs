#[path = "../../build/native.rs"]
mod native;

use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    // Read the current source path when running, since build artifacts may move.
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cpp_dir = manifest_dir.join("cpp");

    println!("cargo:rerun-if-changed=cpp/yuv_ffi.h");

    let includes = native::probe("libyuv", "libyuv", "LIBYUV_STATIC");
    generate_bindings(&cpp_dir, &includes);
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-lib=stdc++");
    }
}

fn generate_bindings(cpp_dir: &Path, includes: &[PathBuf]) {
    let ffi_header = cpp_dir.join("yuv_ffi.h");
    let builder = bindgen::builder()
        .header(ffi_header.to_string_lossy().to_string())
        .clang_args(includes.iter().map(|path| format!("-I{}", path.display())));

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
