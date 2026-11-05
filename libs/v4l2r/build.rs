use std::env;
use std::path::PathBuf;

include!("bindgen.rs");

/// Vendored Linux UAPI include root.
const VENDORED_INCLUDE_DIR: &str = "include";

/// Wrapper file to use as input of bindgen.
const WRAPPER_H: &str = "v4l2r_wrapper.h";

// Fix for https://github.com/rust-lang/rust-bindgen/issues/753
const FIX753_H: &str = "fix753.h";

fn main() {
    let include_root =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` is not set"))
            .join(VENDORED_INCLUDE_DIR);
    let videodev2_h = include_root.join("linux/videodev2.h");

    println!("cargo::rerun-if-changed={}", videodev2_h.display());
    println!("cargo::rerun-if-changed={}", FIX753_H);
    println!("cargo::rerun-if-changed={}", WRAPPER_H);

    let clang_args = vec![
        format!("-I{}", include_root.display()),
        #[cfg(all(feature = "arch64", not(feature = "arch32")))]
        "--target=x86_64-linux-gnu".into(),
        #[cfg(all(feature = "arch32", not(feature = "arch64")))]
        "--target=i686-linux-gnu".into(),
    ];

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
