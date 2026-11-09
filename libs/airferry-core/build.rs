use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=AIRFERRY_NATIVE_DIR");
    println!("cargo:rerun-if-changed=native");
    let target = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let include_paths = if target == "msvc" {
        vcpkg::Config::new()
            .find_package("libjpeg-turbo")
            .expect("Install libjpeg-turbo with vcpkg before building One-KVM")
            .include_paths
    } else {
        pkg_config::Config::new()
            .probe("libturbojpeg")
            .expect("libjpeg-turbo development files are required for built-in AirFerry reception")
            .include_paths
    };
    let native = if let Some(directory) = env::var_os("AIRFERRY_NATIVE_DIR") {
        let directory = PathBuf::from(directory);
        for name in ["libairferry_native.a", "libZXing.a"] {
            println!("cargo:rerun-if-changed={}", directory.join(name).display());
        }
        directory
    } else {
        let includes = include_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(";");
        cmake::Config::new("native")
            .profile("Release")
            .define(
                "AIRFERRY_FIXTURES",
                if env::var_os("CARGO_FEATURE_FIXTURES").is_some() {
                    "ON"
                } else {
                    "OFF"
                },
            )
            .define("TURBOJPEG_INCLUDE_DIRS", includes)
            .build()
            .join("lib")
    };
    println!("cargo:rustc-link-search=native={}", native.display());
    println!("cargo:rustc-link-lib=static=airferry_native");
    println!("cargo:rustc-link-lib=static=ZXing");
    // The JPEG API is already shared with One-KVM's video codecs.
    println!("cargo:rustc-link-lib=turbojpeg");
    if target != "msvc" {
        println!("cargo:rustc-link-lib=stdc++");
    }
    if target == "musl" {
        // Global sysroot -L flags can make rustc bundle the sysroot's GCC-LTO
        // liblzma.a instead of lzma-sys's portable archive. Use a unique name
        // for the bundled archive without changing global compiler/cache flags.
        let source = PathBuf::from(env::var_os("DEP_LZMA_ROOT").expect("bundled XZ archive"))
            .join("liblzma.a");
        let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
        std::fs::copy(&source, output.join("libone_kvm_lzma.a")).expect("copy bundled XZ archive");
        println!("cargo:rerun-if-changed={}", source.display());
        println!("cargo:rustc-link-search=native={}", output.display());
        println!("cargo:rustc-link-lib=static=one_kvm_lzma");
    }
}
