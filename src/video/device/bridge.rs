//! CSI/HDMI bridge helpers: subdev discovery, DV probe, RK628 "fake VGA" filter (must run before `S_FMT` / `STREAMON` on capture — see RK628 driver).

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
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
use v4l2r::ioctl::{
    self, Event as V4l2Event, EventType, IntoErrno, QueryDvTimingsError, SubscribeEventFlags,
};
use v4l2r::nix::errno::Errno;

use crate::video::signal::SignalStatus;

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
    Unavailable,
}

impl ProbeResult {
    pub fn as_status(&self) -> Option<SignalStatus> {
        match self {
            ProbeResult::Locked(_) => None,
            ProbeResult::NoCable => Some(SignalStatus::NoCable),
            ProbeResult::NoSync => Some(SignalStatus::NoSync),
            ProbeResult::OutOfRange => Some(SignalStatus::OutOfRange),
            ProbeResult::NoSignal => Some(SignalStatus::NoSignal),
            ProbeResult::Unavailable => None,
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

const SYSFS_VIDEO4LINUX: &str = "/sys/class/video4linux";
const DEV_ROOT: &str = "/dev";
const MEDIA_ENT_ID_FLAG_NEXT: u32 = 1 << 31;
const MEDIA_LNK_FL_ENABLED: u32 = 1 << 0;
const MEDIA_LNK_FL_LINK_TYPE: u32 = 0xf << 28;
const MEDIA_LNK_FL_DATA_LINK: u32 = 0 << 28;

#[repr(C)]
#[derive(Clone, Copy)]
struct MediaEntityDesc {
    id: u32,
    name: [u8; 32],
    type_: u32,
    revision: u32,
    flags: u32,
    group_id: u32,
    pads: u16,
    links: u16,
    reserved: [u32; 4],
    info: MediaEntityInfo,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MediaDeviceNode {
    major: u32,
    minor: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
union MediaEntityInfo {
    dev: MediaDeviceNode,
    _raw: [u8; 184],
}

impl Default for MediaEntityDesc {
    fn default() -> Self {
        // This mirrors the zero-initialization required by the media UAPI.
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MediaPadDesc {
    entity: u32,
    index: u16,
    flags: u32,
    reserved: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MediaLinkDesc {
    source: MediaPadDesc,
    sink: MediaPadDesc,
    flags: u32,
    reserved: [u32; 2],
}

#[repr(C)]
struct MediaLinksEnum {
    entity: u32,
    pads: *mut MediaPadDesc,
    links: *mut MediaLinkDesc,
    reserved: [u32; 4],
}

nix::ioctl_readwrite!(media_ioc_enum_entities, b'|', 0x01, MediaEntityDesc);
nix::ioctl_readwrite!(media_ioc_enum_links, b'|', 0x02, MediaLinksEnum);

#[derive(Debug, Clone)]
struct MediaGraphEntity {
    id: u32,
    name: String,
    major: u32,
    minor: u32,
    pads: u16,
    links: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MediaGraphLink {
    source: u32,
    sink: u32,
}

#[derive(Debug)]
struct MediaGraph {
    path: PathBuf,
    entities: Vec<MediaGraphEntity>,
    links: Vec<MediaGraphLink>,
}

/// Find the CSI/HDMI bridge that is connected to `video_path` in the same
/// media-controller graph. Name-only global scans are deliberately avoided:
/// boards can expose RK628, native HDMI RX and USB capture at the same time.
pub fn discover_subdev_for_video(video_path: &Path) -> Option<(PathBuf, CsiBridgeKind)> {
    match discover_subdev_for_video_inner(video_path) {
        Ok(Some((path, kind, media_path))) => {
            info!(
                "Discovered CSI bridge subdev for {:?}: {:?} ({:?}) via {:?}",
                video_path, path, kind, media_path
            );
            Some((path, kind))
        }
        Ok(None) => {
            debug!(
                "No connected CSI bridge subdev found in media topology for {:?}",
                video_path
            );
            None
        }
        Err(error) => {
            warn!(
                "Failed to inspect media topology for {:?}: {}",
                video_path, error
            );
            None
        }
    }
}

fn discover_subdev_for_video_inner(
    video_path: &Path,
) -> io::Result<Option<(PathBuf, CsiBridgeKind, PathBuf)>> {
    let video_device = device_numbers(video_path)?;
    let mut graphs = Vec::new();
    let mut first_error = None;

    // A physical device may expose more than one media controller.  Inspect
    // every controller and use the graph that actually contains video_path;
    // choosing the first mediaN node merely moves the old global-scan bug.
    for media_path in media_device_paths()? {
        let graph = File::open(&media_path).and_then(|media| {
            read_media_graph(&media).map(|(entities, links)| MediaGraph {
                path: media_path.clone(),
                entities,
                links,
            })
        });
        match graph {
            Ok(graph) => graphs.push(graph),
            Err(error) => {
                debug!(
                    "Failed to inspect media controller {:?}: {}",
                    media_path, error
                );
                first_error.get_or_insert(error);
            }
        }
    }

    let graph_contains_video = graphs.iter().any(|graph| {
        graph
            .entities
            .iter()
            .any(|entity| (entity.major, entity.minor) == video_device)
    });
    let Some((media_path, entity, kind)) = connected_bridge_in_media_graphs(&graphs, video_device)
    else {
        if !graph_contains_video {
            if let Some(error) = first_error {
                return Err(error);
            }
        }
        return Ok(None);
    };
    let Some(subdev_path) = video4linux_devnode((entity.major, entity.minor))? else {
        return Ok(None);
    };

    if !subdev_path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("v4l-subdev"))
    {
        return Ok(None);
    }

    Ok(Some((subdev_path, kind, media_path.to_path_buf())))
}

fn device_numbers(path: &Path) -> io::Result<(u32, u32)> {
    let rdev = std::fs::metadata(path)?.rdev();
    let major = nix::sys::stat::major(rdev);
    let minor = nix::sys::stat::minor(rdev);
    Ok((major as u32, minor as u32))
}

fn parse_device_numbers(value: &str) -> Option<(u32, u32)> {
    let (major, minor) = value.trim().split_once(':')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

fn video4linux_class_entry(device: (u32, u32)) -> io::Result<Option<PathBuf>> {
    for entry in std::fs::read_dir(SYSFS_VIDEO4LINUX)? {
        let entry = entry?;
        let dev = match std::fs::read_to_string(entry.path().join("dev")) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if parse_device_numbers(&dev) == Some(device) {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
}

fn video4linux_devnode(device: (u32, u32)) -> io::Result<Option<PathBuf>> {
    let Some(class_entry) = video4linux_class_entry(device)? else {
        return Ok(None);
    };
    let Some(name) = class_entry.file_name() else {
        return Ok(None);
    };
    let path = Path::new(DEV_ROOT).join(name);
    Ok(path.exists().then_some(path))
}

fn media_device_paths() -> io::Result<Vec<PathBuf>> {
    let mut media_nodes = std::fs::read_dir(DEV_ROOT)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            let index = media_device_index(name)?;
            Some((index, entry.path()))
        })
        .collect::<Vec<_>>();
    media_nodes.sort_by(|(left_index, left_path), (right_index, right_path)| {
        left_index
            .cmp(right_index)
            .then_with(|| left_path.cmp(right_path))
    });
    Ok(media_nodes.into_iter().map(|(_, path)| path).collect())
}

fn media_device_index(name: &str) -> Option<u32> {
    let suffix = name.strip_prefix("media")?;
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok()
}

fn read_media_graph(media: &File) -> io::Result<(Vec<MediaGraphEntity>, Vec<MediaGraphLink>)> {
    let mut entities = Vec::new();
    let mut previous_id = 0u32;

    loop {
        let mut desc = MediaEntityDesc {
            id: previous_id | MEDIA_ENT_ID_FLAG_NEXT,
            ..Default::default()
        };
        // SAFETY: `desc` has the exact media_entity_desc UAPI layout and is
        // writable for the duration of the ioctl.
        match unsafe { media_ioc_enum_entities(media.as_raw_fd(), &mut desc) } {
            Ok(_) => {}
            Err(Errno::EINVAL) => break,
            Err(error) => return Err(io::Error::from_raw_os_error(error as i32)),
        }
        if desc.id == previous_id || entities.len() >= 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "media entity enumeration did not advance",
            ));
        }
        previous_id = desc.id;

        let nul = desc
            .name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(desc.name.len());
        let name = String::from_utf8_lossy(&desc.name[..nul]).into_owned();
        // SAFETY: the kernel filled the `dev` member of the media UAPI union
        // for V4L2 devnode entities.  Non-devnode entities report zeroes.
        let device = unsafe { desc.info.dev };
        entities.push(MediaGraphEntity {
            id: desc.id,
            name,
            major: device.major,
            minor: device.minor,
            pads: desc.pads,
            links: desc.links,
        });
    }

    let mut graph_links = HashSet::new();
    for entity in &entities {
        let mut pads = vec![MediaPadDesc::default(); entity.pads as usize];
        let mut links = vec![MediaLinkDesc::default(); entity.links as usize];
        let mut request = MediaLinksEnum {
            entity: entity.id,
            pads: if pads.is_empty() {
                std::ptr::null_mut()
            } else {
                pads.as_mut_ptr()
            },
            links: if links.is_empty() {
                std::ptr::null_mut()
            } else {
                links.as_mut_ptr()
            },
            reserved: [0; 4],
        };
        // SAFETY: the vectors provide the number of entries reported by the
        // entity descriptor and remain alive while the kernel fills them.
        unsafe { media_ioc_enum_links(media.as_raw_fd(), &mut request) }
            .map_err(|error| io::Error::from_raw_os_error(error as i32))?;

        for link in links {
            if link.flags & MEDIA_LNK_FL_ENABLED == 0
                || link.flags & MEDIA_LNK_FL_LINK_TYPE != MEDIA_LNK_FL_DATA_LINK
            {
                continue;
            }
            graph_links.insert((link.source.entity, link.sink.entity));
        }
    }

    let mut links = graph_links
        .into_iter()
        .map(|(source, sink)| MediaGraphLink { source, sink })
        .collect::<Vec<_>>();
    links.sort_by_key(|link| (link.source, link.sink));
    Ok((entities, links))
}

fn connected_bridge_entity<'a>(
    entities: &'a [MediaGraphEntity],
    links: &[MediaGraphLink],
    video_device: (u32, u32),
) -> Option<(&'a MediaGraphEntity, CsiBridgeKind)> {
    let start = entities
        .iter()
        .find(|entity| (entity.major, entity.minor) == video_device)?;
    let by_id = entities
        .iter()
        .map(|entity| (entity.id, entity))
        .collect::<HashMap<_, _>>();
    let mut upstream = HashMap::<u32, Vec<u32>>::new();
    for link in links {
        // Media data flows source -> sink.  Starting at a capture video node,
        // only sink -> source traversal can lead to its real input bridge.
        upstream.entry(link.sink).or_default().push(link.source);
    }
    for neighbors in upstream.values_mut() {
        neighbors.sort_unstable();
        neighbors.dedup();
    }

    let mut visited = HashSet::from([start.id]);
    let mut queue = VecDeque::from([start.id]);
    while let Some(id) = queue.pop_front() {
        if id != start.id {
            if let Some(entity) = by_id.get(&id) {
                if entity.major != 0 || entity.minor != 0 {
                    if let Some(kind) = CsiBridgeKind::from_subdev_name(&entity.name) {
                        return Some((entity, kind));
                    }
                }
            }
        }

        if let Some(neighbors) = upstream.get(&id) {
            for neighbor in neighbors {
                if visited.insert(*neighbor) {
                    queue.push_back(*neighbor);
                }
            }
        }
    }
    None
}

fn connected_bridge_in_media_graphs(
    graphs: &[MediaGraph],
    video_device: (u32, u32),
) -> Option<(&Path, &MediaGraphEntity, CsiBridgeKind)> {
    graphs.iter().find_map(|graph| {
        connected_bridge_entity(&graph.entities, &graph.links, video_device)
            .map(|(entity, kind)| (graph.path.as_path(), entity, kind))
    })
}

pub fn open_subdev(path: &Path) -> io::Result<File> {
    File::options()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

/// Drain a non-blocking V4L2 event queue until the driver reports that it is
/// empty.  Both video nodes and subdevices use the same event ioctl contract.
pub fn drain_v4l2_events(fd: &File) -> u32 {
    let mut drained = 0u32;
    loop {
        match ioctl::dqevent::<V4l2Event>(fd) {
            Ok(_event) => {
                drained = drained.saturating_add(1);
                if drained >= 16 {
                    break;
                }
            }
            Err(error) => {
                let message = error.to_string();
                let errno = error.into_errno();
                if errno != Errno::EAGAIN as i32 && errno != Errno::ENOENT as i32 {
                    debug!("Failed to drain V4L2 event queue: {}", message);
                }
                break;
            }
        }
    }
    drained
}

pub fn probe_signal(subdev_fd: &impl AsRawFd, kind: CsiBridgeKind) -> ProbeResult {
    match ioctl::query_dv_timings::<v4l2_dv_timings>(subdev_fd) {
        Ok(timings) => classify_timings(timings, kind),
        Err(error) => classify_query_error(&error, kind),
    }
}

fn classify_query_error(error: &QueryDvTimingsError, kind: CsiBridgeKind) -> ProbeResult {
    match error {
        QueryDvTimingsError::NoLink => ProbeResult::NoCable,
        QueryDvTimingsError::UnstableSignal => ProbeResult::NoSync,
        QueryDvTimingsError::IoctlError(Errno::ERANGE) => ProbeResult::OutOfRange,
        QueryDvTimingsError::IoctlError(Errno::EIO | Errno::EREMOTEIO | Errno::ETIMEDOUT) => {
            ProbeResult::NoSync
        }
        QueryDvTimingsError::Unsupported
        | QueryDvTimingsError::IoctlError(
            Errno::ENOTTY | Errno::EINVAL | Errno::ENOSYS | Errno::EOPNOTSUPP,
        ) if kind == CsiBridgeKind::Unknown => ProbeResult::Unavailable,
        QueryDvTimingsError::Unsupported | QueryDvTimingsError::IoctlError(_) => {
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
            debug!(
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

    let drained = drain_v4l2_events(subdev_fd);
    debug!("subdev source_change drained {} event(s)", drained);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media_graph(
        path: &str,
        entities: Vec<MediaGraphEntity>,
        links: Vec<MediaGraphLink>,
    ) -> MediaGraph {
        MediaGraph {
            path: PathBuf::from(path),
            entities,
            links,
        }
    }

    fn graph_entity(id: u32, name: &str, device: (u32, u32)) -> MediaGraphEntity {
        MediaGraphEntity {
            id,
            name: name.to_string(),
            major: device.0,
            minor: device.1,
            pads: 0,
            links: 0,
        }
    }

    fn graph_link(source: u32, sink: u32) -> MediaGraphLink {
        MediaGraphLink { source, sink }
    }

    #[test]
    fn media_uapi_layout_matches_linux_legacy_api() {
        assert_eq!(std::mem::size_of::<MediaEntityDesc>(), 256);
        assert_eq!(std::mem::size_of::<MediaPadDesc>(), 20);
        assert_eq!(std::mem::size_of::<MediaLinkDesc>(), 52);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<MediaLinksEnum>(), 40);
        #[cfg(target_pointer_width = "32")]
        assert_eq!(std::mem::size_of::<MediaLinksEnum>(), 28);
    }

    #[test]
    fn media_device_names_require_a_numeric_suffix() {
        assert_eq!(media_device_index("media0"), Some(0));
        assert_eq!(media_device_index("media12"), Some(12));
        assert_eq!(media_device_index("media"), None);
        assert_eq!(media_device_index("media-controller"), None);
        assert_eq!(media_device_index("video0"), None);
    }

    #[test]
    fn subdevice_handles_are_non_blocking() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let opened = open_subdev(file.path()).unwrap();
        let flags = unsafe { libc::fcntl(opened.as_raw_fd(), libc::F_GETFL) };
        assert!(flags >= 0);
        assert_ne!(flags & libc::O_NONBLOCK, 0);
    }

    #[test]
    fn media_graph_finds_only_the_connected_rk628() {
        // Captured shape of the RK3588 RKCIF graph:
        // video0 <- mipi-csi2 <- dphy <- RK628.  A second RK628 entity is
        // present in the same synthetic topology but is not connected.
        let entities = vec![
            graph_entity(1, "stream_cif_mipi_id0", (81, 0)),
            graph_entity(45, "rockchip-mipi-csi2", (0, 0)),
            graph_entity(58, "rockchip-csi2-dphy0", (0, 0)),
            graph_entity(63, "m00_b_rk628-csi 3-0050", (81, 16)),
            graph_entity(90, "other-rk628-csi 7-0050", (81, 19)),
        ];
        let links = vec![graph_link(63, 58), graph_link(58, 45), graph_link(45, 1)];

        let (entity, kind) = connected_bridge_entity(&entities, &links, (81, 0)).unwrap();
        assert_eq!(entity.id, 63);
        assert_eq!(kind, CsiBridgeKind::Rk628);
    }

    #[test]
    fn media_graph_search_only_walks_towards_link_sources() {
        let entities = vec![
            graph_entity(1, "stream_cif_mipi_id0", (81, 0)),
            graph_entity(20, "rockchip-mipi-csi2", (0, 0)),
            graph_entity(63, "unrelated-rk628-csi 7-0050", (81, 19)),
        ];
        // Entity 20 is upstream of the capture node.  Entity 63 is downstream
        // of 20 and must not be reached while tracing the capture input.
        let links = vec![graph_link(20, 1), graph_link(20, 63)];

        assert!(connected_bridge_entity(&entities, &links, (81, 0)).is_none());
    }

    #[test]
    fn media_graphs_select_the_controller_containing_the_video_node() {
        let graphs = vec![
            media_graph(
                "/dev/media0",
                vec![
                    graph_entity(1, "other-video", (81, 4)),
                    graph_entity(63, "wrong-rk628-csi", (81, 16)),
                ],
                vec![graph_link(63, 1)],
            ),
            media_graph(
                "/dev/media1",
                vec![
                    graph_entity(10, "stream_cif_mipi_id0", (81, 0)),
                    graph_entity(75, "tc358743 2-000f", (81, 20)),
                ],
                vec![graph_link(75, 10)],
            ),
        ];

        let (path, entity, kind) = connected_bridge_in_media_graphs(&graphs, (81, 0)).unwrap();
        assert_eq!(path, Path::new("/dev/media1"));
        assert_eq!(entity.id, 75);
        assert_eq!(kind, CsiBridgeKind::Tc358743);
    }

    #[test]
    fn media_graph_does_not_attach_an_unrelated_rk628_to_native_hdmirx() {
        let entities = vec![
            graph_entity(1, "rk_hdmirx", (81, 11)),
            graph_entity(63, "m00_b_rk628-csi 3-0050", (81, 16)),
        ];

        assert!(connected_bridge_entity(&entities, &[], (81, 11)).is_none());
    }

    #[test]
    fn media_graph_leaves_usb_capture_without_a_csi_bridge() {
        let entities = vec![
            graph_entity(1, "USB Video: USB Video", (81, 12)),
            graph_entity(8, "Processing 2", (0, 0)),
            graph_entity(11, "Input 1", (0, 0)),
        ];
        let links = vec![graph_link(11, 8), graph_link(8, 1)];

        assert!(connected_bridge_entity(&entities, &links, (81, 12)).is_none());
    }

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

    #[test]
    fn query_errno_mapping_distinguishes_signal_loss_from_unsupported_nodes() {
        assert!(matches!(
            classify_query_error(&QueryDvTimingsError::NoLink, CsiBridgeKind::RkHdmirx),
            ProbeResult::NoCable
        ));
        assert!(matches!(
            classify_query_error(
                &QueryDvTimingsError::UnstableSignal,
                CsiBridgeKind::RkHdmirx
            ),
            ProbeResult::NoSync
        ));
        assert!(matches!(
            classify_query_error(&QueryDvTimingsError::Unsupported, CsiBridgeKind::RkHdmirx),
            ProbeResult::NoSignal
        ));
        assert!(matches!(
            classify_query_error(&QueryDvTimingsError::Unsupported, CsiBridgeKind::Unknown),
            ProbeResult::Unavailable
        ));
    }
}
