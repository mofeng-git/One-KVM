//! USB HID Consumer Control Usage codes
//!
//! Reference: USB HID Usage Tables 1.12, Section 15 (Consumer Page 0x0C)

/// Consumer Control Usage codes for multimedia keys
pub mod usage {
    // Transport Controls
    pub const PLAY_PAUSE: u16 = 0x00CD;
    pub const STOP: u16 = 0x00B7;
    pub const NEXT_TRACK: u16 = 0x00B5;
    pub const PREV_TRACK: u16 = 0x00B6;

    // Volume Controls
    pub const MUTE: u16 = 0x00E2;
    pub const VOLUME_UP: u16 = 0x00E9;
    pub const VOLUME_DOWN: u16 = 0x00EA;
}

/// Check if a usage code is valid
pub fn is_valid_usage(usage: u16) -> bool {
    matches!(
        usage,
        usage::PLAY_PAUSE
            | usage::STOP
            | usage::NEXT_TRACK
            | usage::PREV_TRACK
            | usage::MUTE
            | usage::VOLUME_UP
            | usage::VOLUME_DOWN
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_usage_codes() {
        assert!(is_valid_usage(usage::PLAY_PAUSE));
        assert!(is_valid_usage(usage::MUTE));
        assert!(is_valid_usage(usage::VOLUME_UP));
        assert!(!is_valid_usage(0x0000));
        assert!(!is_valid_usage(0xFFFF));
    }
}
