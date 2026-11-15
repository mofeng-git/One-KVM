use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Shared by capture workers so changing the setting also affects existing pipelines.
#[derive(Clone, Debug, Default)]
pub struct CaptureKeepAlive(Arc<AtomicBool>);

impl CaptureKeepAlive {
    pub(in crate::video) fn set_enabled(&self, enabled: bool) {
        self.0.store(enabled, Ordering::Release);
    }

    pub fn is_enabled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn is_idle(&self, consumers: usize) -> bool {
        consumers == 0 && !self.is_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_the_setting_reaches_existing_capture_workers() {
        let setting = CaptureKeepAlive::default();
        let direct_capture = setting.clone();
        let shared_pipeline = setting.clone();
        assert!(direct_capture.is_idle(0));
        setting.set_enabled(true);
        assert!(!direct_capture.is_idle(0));
        assert!(!shared_pipeline.is_idle(0));
        setting.set_enabled(false);
        assert!(shared_pipeline.is_idle(0));
        assert!(!shared_pipeline.is_idle(1));
    }
}
