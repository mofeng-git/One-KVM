use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::video::signal::SignalStatus;

pub const RK628_SUBDEV_PROBE_TIMEOUT: Duration = Duration::from_millis(3000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsiBridgeKind {
    Rk628,
    RkHdmirx,
    Tc358743,
    Unknown,
}

#[derive(Debug, Clone)]
pub enum ProbeResult {
    Locked(DvTimingsMode),
    NoCable,
    NoSync,
    OutOfRange,
    NoSignal,
}

impl ProbeResult {
    pub fn as_status(&self) -> Option<SignalStatus> {
        match self {
            ProbeResult::Locked(_) => None,
            ProbeResult::NoCable => Some(SignalStatus::NoCable),
            ProbeResult::NoSync => Some(SignalStatus::NoSync),
            ProbeResult::OutOfRange => Some(SignalStatus::OutOfRange),
            ProbeResult::NoSignal => Some(SignalStatus::NoSignal),
        }
    }

    pub fn is_locked(&self) -> bool {
        matches!(self, ProbeResult::Locked(_))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DvTimingsMode {
    pub width: u32,
    pub height: u32,
    pub pixelclock: u64,
    pub fps: Option<f64>,
    pub raw: (),
}

pub fn discover_subdev_for_video(_video_path: &Path) -> Option<(PathBuf, CsiBridgeKind)> {
    None
}

pub fn open_subdev(path: &Path) -> io::Result<File> {
    File::open(path)
}

pub fn probe_signal(_subdev_fd: &File, _kind: CsiBridgeKind) -> ProbeResult {
    ProbeResult::NoSignal
}

pub fn probe_signal_thread_timeout(
    _subdev_fd: &File,
    _kind: CsiBridgeKind,
    _timeout: Duration,
) -> Option<ProbeResult> {
    Some(ProbeResult::NoSignal)
}

pub fn apply_dv_timings(_subdev_fd: &File, _timings: ()) {}

pub fn subscribe_source_change(_subdev_fd: &File) -> io::Result<()> {
    Ok(())
}

pub fn wait_source_change(_subdev_fd: &File, _timeout: Duration) -> io::Result<bool> {
    Ok(false)
}
