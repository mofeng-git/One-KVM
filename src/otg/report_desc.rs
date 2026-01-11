//! HID Report Descriptors

/// Keyboard HID Report Descriptor (no LED output - saves 1 endpoint)
/// Report format (8 bytes input):
///   [0] Modifier keys (8 bits)
///   [1] Reserved
///   [2-7] Key codes (6 keys)
pub const KEYBOARD: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x06, // Usage (Keyboard)
    0xA1, 0x01, // Collection (Application)
    // Modifier keys input (8 bits)
    0x05, 0x07, //   Usage Page (Key Codes)
    0x19, 0xE0, //   Usage Minimum (224) - Left Control
    0x29, 0xE7, //   Usage Maximum (231) - Right GUI
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x08, //   Report Count (8)
    0x81, 0x02, //   Input (Data, Variable, Absolute) - Modifier byte
    // Reserved byte
    0x95, 0x01, //   Report Count (1)
    0x75, 0x08, //   Report Size (8)
    0x81, 0x01, //   Input (Constant) - Reserved byte
    // Key array (6 bytes)
    0x95, 0x06, //   Report Count (6)
    0x75, 0x08, //   Report Size (8)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0xFF, 0x00, // Logical Maximum (255)
    0x05, 0x07, //   Usage Page (Key Codes)
    0x19, 0x00, //   Usage Minimum (0)
    0x2A, 0xFF, 0x00, // Usage Maximum (255)
    0x81, 0x00, //   Input (Data, Array) - Key array (6 keys)
    0xC0, // End Collection
];

/// Relative Mouse HID Report Descriptor (4 bytes report)
/// Report format:
///   [0] Buttons (5 bits) + padding (3 bits)
///   [1] X movement (signed 8-bit)
///   [2] Y movement (signed 8-bit)
///   [3] Wheel (signed 8-bit)
pub const MOUSE_RELATIVE: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x02, // Usage (Mouse)
    0xA1, 0x01, // Collection (Application)
    0x09, 0x01, //   Usage (Pointer)
    0xA1, 0x00, //   Collection (Physical)
    // Buttons (5 bits)
    0x05, 0x09, //     Usage Page (Button)
    0x19, 0x01, //     Usage Minimum (1)
    0x29, 0x05, //     Usage Maximum (5) - 5 buttons
    0x15, 0x00, //     Logical Minimum (0)
    0x25, 0x01, //     Logical Maximum (1)
    0x95, 0x05, //     Report Count (5)
    0x75, 0x01, //     Report Size (1)
    0x81, 0x02, //     Input (Data, Variable, Absolute) - Button bits
    // Padding (3 bits)
    0x95, 0x01, //     Report Count (1)
    0x75, 0x03, //     Report Size (3)
    0x81, 0x01, //     Input (Constant) - Padding
    // X, Y movement
    0x05, 0x01, //     Usage Page (Generic Desktop)
    0x09, 0x30, //     Usage (X)
    0x09, 0x31, //     Usage (Y)
    0x15, 0x81, //     Logical Minimum (-127)
    0x25, 0x7F, //     Logical Maximum (127)
    0x75, 0x08, //     Report Size (8)
    0x95, 0x02, //     Report Count (2)
    0x81, 0x06, //     Input (Data, Variable, Relative) - X, Y
    // Wheel
    0x09, 0x38, //     Usage (Wheel)
    0x15, 0x81, //     Logical Minimum (-127)
    0x25, 0x7F, //     Logical Maximum (127)
    0x75, 0x08, //     Report Size (8)
    0x95, 0x01, //     Report Count (1)
    0x81, 0x06, //     Input (Data, Variable, Relative) - Wheel
    0xC0, //   End Collection
    0xC0, // End Collection
];

/// Absolute Mouse HID Report Descriptor (6 bytes report)
/// Report format:
///   [0] Buttons (5 bits) + padding (3 bits)
///   [1-2] X position (16-bit, 0-32767)
///   [3-4] Y position (16-bit, 0-32767)
///   [5] Wheel (signed 8-bit)
pub const MOUSE_ABSOLUTE: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x02, // Usage (Mouse)
    0xA1, 0x01, // Collection (Application)
    0x09, 0x01, //   Usage (Pointer)
    0xA1, 0x00, //   Collection (Physical)
    // Buttons (5 bits)
    0x05, 0x09, //     Usage Page (Button)
    0x19, 0x01, //     Usage Minimum (1)
    0x29, 0x05, //     Usage Maximum (5) - 5 buttons
    0x15, 0x00, //     Logical Minimum (0)
    0x25, 0x01, //     Logical Maximum (1)
    0x95, 0x05, //     Report Count (5)
    0x75, 0x01, //     Report Size (1)
    0x81, 0x02, //     Input (Data, Variable, Absolute) - Button bits
    // Padding (3 bits)
    0x95, 0x01, //     Report Count (1)
    0x75, 0x03, //     Report Size (3)
    0x81, 0x01, //     Input (Constant) - Padding
    // X position (16-bit absolute)
    0x05, 0x01, //     Usage Page (Generic Desktop)
    0x09, 0x30, //     Usage (X)
    0x16, 0x00, 0x00, // Logical Minimum (0)
    0x26, 0xFF, 0x7F, // Logical Maximum (32767)
    0x75, 0x10, //     Report Size (16)
    0x95, 0x01, //     Report Count (1)
    0x81, 0x02, //     Input (Data, Variable, Absolute) - X
    // Y position (16-bit absolute)
    0x09, 0x31, //     Usage (Y)
    0x16, 0x00, 0x00, // Logical Minimum (0)
    0x26, 0xFF, 0x7F, // Logical Maximum (32767)
    0x75, 0x10, //     Report Size (16)
    0x95, 0x01, //     Report Count (1)
    0x81, 0x02, //     Input (Data, Variable, Absolute) - Y
    // Wheel
    0x09, 0x38, //     Usage (Wheel)
    0x15, 0x81, //     Logical Minimum (-127)
    0x25, 0x7F, //     Logical Maximum (127)
    0x75, 0x08, //     Report Size (8)
    0x95, 0x01, //     Report Count (1)
    0x81, 0x06, //     Input (Data, Variable, Relative) - Wheel
    0xC0, //   End Collection
    0xC0, // End Collection
];

/// Consumer Control HID Report Descriptor (2 bytes report)
/// Report format:
///   [0-1] Consumer Control Usage (16-bit little-endian)
/// Supports: Play/Pause, Stop, Next/Prev Track, Mute, Volume Up/Down, etc.
pub const CONSUMER_CONTROL: &[u8] = &[
    0x05, 0x0C, // Usage Page (Consumer)
    0x09, 0x01, // Usage (Consumer Control)
    0xA1, 0x01, // Collection (Application)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0xFF, 0x03, //   Logical Maximum (1023)
    0x19, 0x00, //   Usage Minimum (0)
    0x2A, 0xFF, 0x03, //   Usage Maximum (1023)
    0x75, 0x10, //   Report Size (16)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x00, //   Input (Data, Array)
    0xC0, // End Collection
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_descriptor_sizes() {
        assert!(!KEYBOARD.is_empty());
        assert!(!MOUSE_RELATIVE.is_empty());
        assert!(!MOUSE_ABSOLUTE.is_empty());
        assert!(!CONSUMER_CONTROL.is_empty());
    }
}
