// This file defines the customizations to the bindgen builder used to generate the v4l2r
// bindings.
//
// It is meant to be included from `build.rs`.

#[derive(Debug)]
/// Workaround for https://github.com/rust-lang/rust-bindgen/issues/753.
pub struct Fix753;

impl bindgen::callbacks::ParseCallbacks for Fix753 {
    fn item_name(&self, item_info: bindgen::callbacks::ItemInfo<'_>) -> Option<String> {
        Some(item_info.name.trim_start_matches("Fix753_").to_owned())
    }
}

fn v4l2r_bindgen_builder(builder: bindgen::Builder) -> bindgen::Builder {
    builder
        .parse_callbacks(Box::new(Fix753))
        .derive_default(true)
}
