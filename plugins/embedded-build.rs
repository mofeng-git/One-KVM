use std::{env, fs, path::PathBuf};

fn embed_client(variable: &str) {
    println!("cargo:rerun-if-env-changed={variable}");
    println!("cargo:rerun-if-changed=../embedded-build.rs");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("embedded-client");
    if let Some(path) = env::var_os(variable) {
        let path = PathBuf::from(path);
        let path = if path.is_absolute() {
            path
        } else {
            PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("../..")
                .join(path)
        };
        println!("cargo:rerun-if-changed={}", path.display());
        fs::copy(path, out).expect("Copy bundled upstream client");
    } else {
        fs::write(out, []).expect("Create empty upstream client build placeholder");
    }
}
