// This file defines the customizations to the bindgen builder used to generate the v4l2r
// bindings.
//
// It is meant to be included from `lib/build.rs` and `android/build.rs`.

#[derive(Debug)]
/// Workaround for https://github.com/rust-lang/rust-bindgen/issues/753.
pub struct Fix753;

impl bindgen::callbacks::ParseCallbacks for Fix753 {
    fn item_name(&self, original_item_name: &str) -> Option<String> {
        Some(original_item_name.trim_start_matches("Fix753_").to_owned())
    }
}

fn v4l2r_bindgen_builder(builder: bindgen::Builder) -> bindgen::Builder {
    builder
        .parse_callbacks(Box::new(Fix753))
        .derive_partialeq(true)
        .derive_eq(true)
        .derive_default(true)
}
