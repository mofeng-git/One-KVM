//! CSI/HDMI bridge helpers: subdev discovery, DV probe, RK628 "fake VGA" filter (must run before `S_FMT` / `STREAMON` on capture — see RK628 driver).

use std::fs::File;
use std::io;
use std::os::fd::{AsFd, AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use libc;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use tracing::{debug, info, warn};
use v4l2r::bindings::{
    v4l2_bt_timings, v4l2_dv_timings, V4L2_DV_BT_656_1120, V4L2_DV_FL_HAS_CEA861_VIC,
};
use v4l2r::ioctl::{self, Event as V4l2Event, EventType, QueryDvTimingsError, SubscribeEventFlags};
use v4l2r::nix::errno::Errno;

use crate::video::SignalStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsiBridgeKind {
    Rk628,
    RkHdmirx,
    Tc358743,
    Unknown,
}

impl CsiBridgeKind {
    fn from_subdev_name(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.contains("rk628") {
            Some(Self::Rk628)
        } else if lower.contains("hdmirx") || lower.contains("hdmi-rx") {
            Some(Self::RkHdmirx)
        } else if lower.contains("tc358743") || lower.contains("tc358746") {
            Some(Self::Tc358743)
        } else {
            None
        }
    }

    fn has_no_signal_fingerprint(self) -> bool {
        matches!(self, Self::Rk628)
    }
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

/// Scalar copy of BT timings (avoids unaligned refs into packed union).
#[derive(Clone, Copy)]
pub struct DvTimingsMode {
    pub width: u32,
    pub height: u32,
    pub pixelclock: u64,
    pub fps: Option<f64>,
    pub raw: v4l2_dv_timings,
}

impl std::fmt::Debug for DvTimingsMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DvTimingsMode")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pixelclock", &self.pixelclock)
            .field("fps", &self.fps)
            .finish()
    }
}

/// Heuristic: scan `/sys/class/video4linux/v4l-subdev*` names for rk628 / hdmirx / tc358743.
pub fn discover_subdev_for_video(video_path: &Path) -> Option<(PathBuf, CsiBridgeKind)> {
    let sysfs_base = Path::new("/sys/class/video4linux");
    let entries = std::fs::read_dir(sysfs_base).ok()?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("v4l-subdev") {
            continue;
        }
        let Some(kind) = read_sysfs_name(&entry.path())
            .as_deref()
            .and_then(CsiBridgeKind::from_subdev_name)
        else {
            continue;
        };
        let dev_path = PathBuf::from("/dev").join(&*name_str);
        if dev_path.exists() {
            info!(
                "Discovered CSI bridge subdev for {:?}: {:?} ({:?})",
                video_path, dev_path, kind
            );
            return Some((dev_path, kind));
        }
    }
    debug!(
        "No CSI bridge subdev found in /sys/class/video4linux for {:?}",
        video_path
    );
    None
}

fn read_sysfs_name(subdev_sysfs: &Path) -> Option<String> {
    std::fs::read_to_string(subdev_sysfs.join("name"))
        .ok()
        .map(|s| s.trim().to_string())
}

pub fn open_subdev(path: &Path) -> io::Result<File> {
    File::options().read(true).write(true).open(path)
}

pub fn probe_signal(subdev_fd: &impl AsRawFd, kind: CsiBridgeKind) -> ProbeResult {
    match ioctl::query_dv_timings::<v4l2_dv_timings>(subdev_fd) {
        Ok(timings) => classify_timings(timings, kind),
        Err(QueryDvTimingsError::NoLink) => ProbeResult::NoCable,
        Err(QueryDvTimingsError::UnstableSignal) => ProbeResult::NoSync,
        Err(QueryDvTimingsError::IoctlError(Errno::ERANGE)) => ProbeResult::OutOfRange,
        Err(QueryDvTimingsError::IoctlError(Errno::EIO | Errno::EREMOTEIO | Errno::ETIMEDOUT)) => {
            ProbeResult::NoSync
        }
        Err(QueryDvTimingsError::Unsupported) | Err(QueryDvTimingsError::IoctlError(_)) => {
            ProbeResult::NoSignal
        }
    }
}

/// RK628 can block `QUERY_DV_TIMINGS` for seconds; probe uses a dup + timeout.
pub const RK628_SUBDEV_PROBE_TIMEOUT: Duration = Duration::from_millis(3000);

pub fn probe_signal_thread_timeout(
    subdev_fd: &impl AsRawFd,
    kind: CsiBridgeKind,
    limit: Duration,
) -> Option<ProbeResult> {
    let raw = subdev_fd.as_raw_fd();
    let dup_fd = unsafe { libc::dup(raw) };
    if dup_fd < 0 {
        warn!(
            "dup(subdev) for threaded DV probe failed: {}",
            io::Error::last_os_error()
        );
        return None;
    }
    let dup_file = unsafe { File::from_raw_fd(dup_fd) };
    let (tx, rx) = mpsc::channel::<ProbeResult>();
    let handle = thread::spawn(move || {
        let probe = probe_signal(&dup_file, kind);
        let _ = tx.send(probe);
    });
    match rx.recv_timeout(limit) {
        Ok(r) => {
            let _ = handle.join();
            Some(r)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            warn!(
                "QUERY_DV_TIMINGS exceeded {:?} (RK628 HDMI mode change?) — abandoning probe thread",
                limit
            );
            drop(handle);
            None
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = handle.join();
            None
        }
    }
}

fn classify_timings(timings: v4l2_dv_timings, kind: CsiBridgeKind) -> ProbeResult {
    let timings_type: u32 = timings.type_;
    if timings_type != V4L2_DV_BT_656_1120 {
        warn!(
            "QUERY_DV_TIMINGS returned unexpected type {}, treating as NoSignal",
            timings_type
        );
        return ProbeResult::NoSignal;
    }

    let bt: v4l2_bt_timings = unsafe { timings.__bindgen_anon_1.bt };
    let width: u32 = bt.width;
    let height: u32 = bt.height;
    let pixelclock: u64 = bt.pixelclock;

    if width == 0 || height == 0 || width <= 64 || height <= 64 {
        return ProbeResult::NoSignal;
    }

    if kind.has_no_signal_fingerprint() && is_rk628_no_signal_fingerprint(&bt) {
        debug!(
            "RK628 reports synthetic {}x{} @ {} Hz VGA fingerprint → NoSignal",
            width, height, pixelclock
        );
        return ProbeResult::NoSignal;
    }

    let total_h: u64 = (width + bt.hfrontporch + bt.hsync + bt.hbackporch) as u64;
    let total_v: u64 = (height + bt.vfrontporch + bt.vsync + bt.vbackporch) as u64;
    let fps = if total_h > 0 && total_v > 0 && pixelclock > 0 {
        Some(pixelclock as f64 / (total_h as f64 * total_v as f64))
    } else {
        None
    };

    ProbeResult::Locked(DvTimingsMode {
        width,
        height,
        pixelclock,
        fps,
        raw: timings,
    })
}

/// RK628 returns DMT 640x480 @ ~25.175 MHz, VIC=1 when unlocked; do not stream on that.
fn is_rk628_no_signal_fingerprint(bt: &v4l2_bt_timings) -> bool {
    let width: u32 = bt.width;
    let height: u32 = bt.height;
    let pixelclock: u64 = bt.pixelclock;
    let flags: u32 = bt.flags;
    let vic: u8 = bt.cea861_vic;

    if width != 640 || height != 480 {
        return false;
    }
    let pclk_matches = (pixelclock as i64 - 25_175_000).abs() < 50_000;
    let has_vic_flag = flags & V4L2_DV_FL_HAS_CEA861_VIC != 0;
    pclk_matches && has_vic_flag && vic == 1
}

pub fn apply_dv_timings(subdev_fd: &impl AsRawFd, timings: v4l2_dv_timings) {
    match ioctl::s_dv_timings::<_, v4l2_dv_timings>(subdev_fd, timings) {
        Ok(_) => debug!("S_DV_TIMINGS ok on subdev"),
        Err(e) => debug!(
            "S_DV_TIMINGS failed on subdev ({}), continuing with queried mode",
            e
        ),
    }
}

pub fn subscribe_source_change(subdev_fd: &impl AsRawFd) -> io::Result<()> {
    ioctl::subscribe_event(
        subdev_fd,
        EventType::SourceChange(0),
        SubscribeEventFlags::empty(),
    )
    .map_err(|e| io::Error::other(format!("subscribe_event(SOURCE_CHANGE): {}", e)))
}

/// `Ok(true)` if a SOURCE_CHANGE was drained; `Ok(false)` on timeout.
pub fn wait_source_change(subdev_fd: &File, timeout: Duration) -> io::Result<bool> {
    let mut fds = [PollFd::new(subdev_fd.as_fd(), PollFlags::POLLPRI)];
    let timeout_ms = timeout.as_millis().min(u16::MAX as u128) as u16;
    let ready = poll(&mut fds, PollTimeout::from(timeout_ms))?;
    if ready == 0 {
        return Ok(false);
    }
    if let Some(revents) = fds[0].revents() {
        if !revents.contains(PollFlags::POLLPRI) {
            return Ok(false);
        }
    }

    let mut drained = 0u32;
    while let Ok(_ev) = ioctl::dqevent::<V4l2Event>(subdev_fd) {
        drained = drained.saturating_add(1);
        if drained >= 16 {
            break;
        }
    }
    debug!("subdev source_change drained {} event(s)", drained);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rk628_fingerprint_matches_vga() {
        let mut bt: v4l2_bt_timings = unsafe { std::mem::zeroed() };
        bt.width = 640;
        bt.height = 480;
        bt.pixelclock = 25_175_000;
        bt.flags = V4L2_DV_FL_HAS_CEA861_VIC;
        bt.cea861_vic = 1;
        assert!(is_rk628_no_signal_fingerprint(&bt));
    }

    #[test]
    fn rk628_fingerprint_rejects_real_1080p() {
        let mut bt: v4l2_bt_timings = unsafe { std::mem::zeroed() };
        bt.width = 1920;
        bt.height = 1080;
        bt.pixelclock = 148_500_000;
        bt.flags = V4L2_DV_FL_HAS_CEA861_VIC;
        bt.cea861_vic = 16;
        assert!(!is_rk628_no_signal_fingerprint(&bt));
    }

    #[test]
    fn rk628_fingerprint_rejects_real_vga_without_vic() {
        // A hypothetical legit VGA source would *not* carry the CEA VIC
        // flag from the bridge (RK628 sets it synthetically when unlocked).
        let mut bt: v4l2_bt_timings = unsafe { std::mem::zeroed() };
        bt.width = 640;
        bt.height = 480;
        bt.pixelclock = 25_175_000;
        bt.flags = 0;
        bt.cea861_vic = 0;
        assert!(!is_rk628_no_signal_fingerprint(&bt));
    }

    #[test]
    fn from_subdev_name_recognises_known_bridges() {
        assert_eq!(
            CsiBridgeKind::from_subdev_name("rk628-csi-v4l2 9-0051"),
            Some(CsiBridgeKind::Rk628)
        );
        assert_eq!(
            CsiBridgeKind::from_subdev_name("rk-hdmirx-ctrl"),
            Some(CsiBridgeKind::RkHdmirx)
        );
        assert_eq!(
            CsiBridgeKind::from_subdev_name("tc358743 2-000f"),
            Some(CsiBridgeKind::Tc358743)
        );
        assert_eq!(CsiBridgeKind::from_subdev_name("mystery"), None);
    }
}
