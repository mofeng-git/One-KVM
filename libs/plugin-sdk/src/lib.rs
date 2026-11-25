#[cfg(feature = "embedded")]
pub mod embedded;
pub mod h264_bitstream;
pub mod h265_payloader;
pub mod keyboard;
pub mod types;

#[cfg(feature = "runtime")]
pub mod net;
#[cfg(feature = "runtime")]
pub mod runtime;
#[cfg(feature = "runtime")]
mod runtime_error;

use std::collections::BTreeMap;

use bytes::{BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const ABI: u32 = 1;
pub const MAGIC: &[u8] = b"ONEPKG1\n";
pub const MAX_MANIFEST: usize = 128 * 1024;
pub const MAX_PACKAGE: usize = 192 * 1024 * 1024;
pub const MAX_FRAME: usize = 32 * 1024 * 1024;
/// Short-lived mode for reporting manifest fields available without a running service.
pub const INSPECT_ARG: &str = "--one-kvm-inspect";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Video,
    VideoControl,
    Audio,
    Hid,
    Events,
    Images,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    Mjpeg,
    H264,
    H265,
    Vp8,
    Vp9,
    Opus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub abi: u32,
    pub id: String,
    pub name: LocalizedText,
    pub version: String,
    pub author: String,
    pub os: String,
    pub arch: String,
    #[serde(default)]
    pub description: LocalizedText,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    pub menu: Menu,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolbar_icon: Option<ToolbarIcon>,
    #[serde(default)]
    pub config_schema: Vec<Field>,
    #[serde(default)]
    pub status_fields: Vec<StatusField>,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub files: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub files_in_images: bool,
    #[serde(default)]
    pub default_enabled: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarIcon {
    ScanLine,
    Wrench,
}

/// A display string or translations keyed by BCP 47 language tags.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LocalizedText {
    Plain(String),
    Translations(BTreeMap<String, String>),
}

impl Default for LocalizedText {
    fn default() -> Self {
        Self::Plain(String::new())
    }
}

impl LocalizedText {
    pub fn has_text(&self) -> bool {
        match self {
            Self::Plain(text) => !text.trim().is_empty(),
            Self::Translations(translations) => {
                translations.values().any(|text| !text.trim().is_empty())
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Menu {
    Id(String),
    Named { id: String, label: LocalizedText },
}

impl Menu {
    pub fn id(&self) -> &str {
        match self {
            Self::Id(id) | Self::Named { id, .. } => id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Field {
    pub key: String,
    pub label: LocalizedText,
    pub kind: FieldKind,
    #[serde(default)]
    pub default: Value,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub choice_labels: BTreeMap<String, LocalizedText>,
    #[serde(default)]
    pub min: Option<i64>,
    #[serde(default)]
    pub max: Option<i64>,
    #[serde(default)]
    pub when: Option<FieldCondition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FieldCondition {
    pub key: String,
    pub equals: Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Password,
    Textarea,
    Integer,
    Boolean,
    Select,
    StringList,
}

impl Field {
    pub fn accepts(&self, value: &Value) -> bool {
        match self.kind {
            FieldKind::Text | FieldKind::Password | FieldKind::Textarea => value.is_string(),
            FieldKind::Integer => value.as_i64().is_some_and(|number| {
                self.min.is_none_or(|min| number >= min) && self.max.is_none_or(|max| number <= max)
            }),
            FieldKind::Boolean => value.is_boolean(),
            FieldKind::Select => value
                .as_str()
                .is_some_and(|s| self.choices.iter().any(|choice| choice == s)),
            FieldKind::StringList => value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_string)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusField {
    pub key: String,
    pub label: LocalizedText,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub value_labels: BTreeMap<String, LocalizedText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<StatusFormat>,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub available_offline: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusFormat {
    Percent,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Action {
    pub id: String,
    pub label: LocalizedText,
    #[serde(default)]
    pub confirm: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub when_status: Vec<FieldCondition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Status {
    #[serde(default)]
    pub fields: Map<String, Value>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub mode: Codec,
    pub capture_format: Option<String>,
    pub ready: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FrameHeader {
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub sequence: u64,
    pub pts_ms: u64,
    pub duration_us: u64,
    pub keyframe: bool,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub header: FrameHeader,
    pub data: Bytes,
}

impl Frame {
    // Each stream record is [metadata length (u32 BE), data length (u32 BE), JSON, payload].
    // Return separate chunks so sending frame data keeps its original reference-counted buffer.
    pub fn into_chunks(self) -> Option<[Bytes; 2]> {
        let metadata = serde_json::to_vec(&self.header).ok()?;
        if self.data.len() > MAX_FRAME {
            return None;
        }
        let mut prefix = BytesMut::with_capacity(8 + metadata.len());
        prefix.put_u32(metadata.len() as u32);
        prefix.put_u32(self.data.len() as u32);
        prefix.extend_from_slice(&metadata);
        Some([prefix.freeze(), self.data])
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bitrate {
    Speed,
    Balanced,
    Quality,
}

#[cfg(test)]
mod manifest_tests {
    use super::*;

    #[test]
    fn reads_manifest_metadata_and_menu_declarations() {
        for (name, menu, expected) in [
            (
                serde_json::json!("Example"),
                serde_json::json!("remote_access"),
                "remote_access",
            ),
            (
                serde_json::json!({"en-US": "Example", "zh-CN": "示例"}),
                serde_json::json!({"id": "test-tools", "label": {"zh-CN": "测试工具"}}),
                "test-tools",
            ),
        ] {
            let manifest: Manifest = serde_json::from_value(serde_json::json!({
                "id": "example", "name": name, "abi": ABI, "version": "1",
                "author": "Example Studio",
                "os": "linux", "arch": "arm", "menu": menu,
            }))
            .unwrap();
            assert!(manifest.name.has_text());
            assert_eq!(manifest.author, "Example Studio");
            assert_eq!(manifest.menu.id(), expected);
        }
    }
}
