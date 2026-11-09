//! Platform-agnostic monotonic time for the transfer engine.
//!
//! `std::time::SystemTime::now()` panics on `wasm32-unknown-unknown` (the
//! platform has no clock in core). On WASM we read the browser clock via
//! `js_sys::Date`; on native we use `SystemTime`. Both return UNIX-epoch
//! milliseconds, which is all the engine needs for timestamps and stats.

/// Current UNIX epoch time in milliseconds. Works on native + WASM.
pub fn now_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        // js_sys::Date::now() returns f64 milliseconds since epoch.
        js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}
