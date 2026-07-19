use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

#[cfg(target_os = "linux")]
mod platform {
    use super::{Backend, Device, DiscoveredWatchdog};
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Write};
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    use std::path::{Path, PathBuf};

    const WDIOC_GETSUPPORT: libc::c_ulong = 0x8028_5700;
    const WDIOC_SETOPTIONS: libc::c_ulong = 0x8004_5704;
    const WDIOC_KEEPALIVE: libc::c_ulong = 0x8004_5705;
    const WDIOC_GETTIMEOUT: libc::c_ulong = 0x8004_5707;
    const WDIOS_DISABLECARD: libc::c_int = 0x0001;
    const WDIOF_MAGICCLOSE: u32 = 0x0100;

    #[repr(C)]
    #[derive(Default)]
    struct WatchdogInfo {
        options: u32,
        firmware_version: u32,
        identity: [u8; 32],
    }

    pub struct LinuxBackend {
        sys_root: PathBuf,
        dev_root: PathBuf,
    }

    impl Default for LinuxBackend {
        fn default() -> Self {
            Self {
                sys_root: PathBuf::from("/sys/class/watchdog"),
                dev_root: PathBuf::from("/dev"),
            }
        }
    }

    impl Backend for LinuxBackend {
        fn discover(&self) -> io::Result<Vec<DiscoveredWatchdog>> {
            discover_at(&self.sys_root, &self.dev_root)
        }

        fn open(&self, path: &Path) -> io::Result<Box<dyn Device>> {
            let file = OpenOptions::new().write(true).open(path)?;
            let mut info = WatchdogInfo::default();
            let supports_magic_close = unsafe {
                libc::ioctl(
                    std::os::fd::AsRawFd::as_raw_fd(&file),
                    WDIOC_GETSUPPORT,
                    &mut info,
                ) == 0
                    && info.options & WDIOF_MAGICCLOSE != 0
            };
            Ok(Box::new(LinuxDevice {
                file,
                supports_magic_close,
                nowayout: self.device_nowayout(path),
            }))
        }
    }

    impl LinuxBackend {
        fn device_nowayout(&self, path: &Path) -> Option<bool> {
            let direct_index = path
                .file_name()
                .and_then(|name| watchdog_index(&name.to_string_lossy()));
            if let Some(index) = direct_index {
                return read_trimmed(&self.sys_root.join(format!("watchdog{index}/nowayout")))
                    .and_then(|value| parse_boolean_flag(&value));
            }

            discover_at(&self.sys_root, &self.dev_root)
                .ok()
                .and_then(|devices| {
                    devices.into_iter().find(|device| {
                        device
                            .paths
                            .iter()
                            .any(|candidate| same_file(candidate, path))
                    })
                })
                .and_then(|device| {
                    read_trimmed(
                        &self
                            .sys_root
                            .join(format!("watchdog{}/nowayout", device.index)),
                    )
                })
                .and_then(|value| parse_boolean_flag(&value))
        }
    }

    struct LinuxDevice {
        file: File,
        supports_magic_close: bool,
        nowayout: Option<bool>,
    }

    impl Device for LinuxDevice {
        fn keep_alive(&mut self) -> io::Result<()> {
            let result = unsafe {
                libc::ioctl(
                    std::os::fd::AsRawFd::as_raw_fd(&self.file),
                    WDIOC_KEEPALIVE,
                    0,
                )
            };
            if result == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        }

        fn timeout(&mut self) -> io::Result<u32> {
            let mut timeout: libc::c_int = 0;
            let result = unsafe {
                libc::ioctl(
                    std::os::fd::AsRawFd::as_raw_fd(&self.file),
                    WDIOC_GETTIMEOUT,
                    &mut timeout,
                )
            };
            if result == 0 && timeout > 0 {
                Ok(timeout as u32)
            } else if result == 0 {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "watchdog reported a zero timeout",
                ))
            } else {
                Err(io::Error::last_os_error())
            }
        }

        fn disable(&mut self) -> io::Result<()> {
            if self.nowayout == Some(true) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "watchdog nowayout is enabled",
                ));
            }
            let mut option = WDIOS_DISABLECARD;
            let result = unsafe {
                libc::ioctl(
                    std::os::fd::AsRawFd::as_raw_fd(&self.file),
                    WDIOC_SETOPTIONS,
                    &mut option,
                )
            };
            if result == 0 {
                return Ok(());
            }

            let ioctl_error = io::Error::last_os_error();
            if self.supports_magic_close && self.nowayout == Some(false) {
                self.file.write_all(b"V")?;
                self.file.flush()
            } else if self.supports_magic_close {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "watchdog nowayout state cannot be verified",
                ))
            } else {
                Err(ioctl_error)
            }
        }
    }

    fn watchdog_index(name: &str) -> Option<u32> {
        name.strip_prefix("watchdog")?.parse().ok()
    }

    fn parse_boolean_flag(value: &str) -> Option<bool> {
        match value {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
    }

    fn read_trimmed(path: &Path) -> Option<String> {
        fs::read_to_string(path)
            .ok()
            .map(|value| value.trim().to_string())
    }

    fn path_marker(path: &Path) -> Option<String> {
        fs::canonicalize(path)
            .ok()
            .or_else(|| fs::read_link(path).ok())
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
    }

    fn is_softdog(entry: &Path) -> bool {
        let mut markers = Vec::new();
        for name in ["identity", "name"] {
            if let Some(value) = read_trimmed(&entry.join(name)) {
                markers.push(value);
            }
        }
        for path in [
            entry.join("device/driver"),
            entry.join("device/driver/module"),
        ] {
            if let Some(value) = path_marker(&path) {
                markers.push(value);
            }
        }

        markers.into_iter().any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("softdog") || value.contains("software watchdog")
        })
    }

    fn same_file(left: &Path, right: &Path) -> bool {
        if let (Ok(left), Ok(right)) = (fs::canonicalize(left), fs::canonicalize(right)) {
            if left == right {
                return true;
            }
        }
        match (fs::metadata(left), fs::metadata(right)) {
            (Ok(left), Ok(right)) => {
                if left.file_type().is_char_device() && right.file_type().is_char_device() {
                    left.rdev() == right.rdev()
                } else {
                    left.dev() == right.dev() && left.ino() == right.ino()
                }
            }
            _ => false,
        }
    }

    fn alias_matches_sysfs(entry: &Path, alias: &Path) -> bool {
        let Some(dev) = read_trimmed(&entry.join("dev")) else {
            return false;
        };
        let Some((major, minor)) = dev.split_once(':') else {
            return false;
        };
        let (Ok(major), Ok(minor), Ok(metadata)) = (
            major.parse::<u32>(),
            minor.parse::<u32>(),
            fs::metadata(alias),
        ) else {
            return false;
        };
        metadata.file_type().is_char_device()
            && libc::major(metadata.rdev()) == major
            && libc::minor(metadata.rdev()) == minor
    }

    pub(super) fn discover_at(
        sys_root: &Path,
        dev_root: &Path,
    ) -> io::Result<Vec<DiscoveredWatchdog>> {
        let entries = match fs::read_dir(sys_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let alias = dev_root.join("watchdog");
        let mut devices = Vec::new();

        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(index) = watchdog_index(&name) else {
                continue;
            };
            if is_softdog(&entry.path()) {
                continue;
            }

            let numbered = dev_root.join(name.as_ref());
            let mut paths = Vec::new();
            if numbered.exists() {
                paths.push(numbered.clone());
            }
            if alias.exists()
                && (same_file(&numbered, &alias)
                    || (!numbered.exists() && alias_matches_sysfs(&entry.path(), &alias)))
                && !paths.iter().any(|path| same_file(path, &alias))
            {
                paths.push(alias.clone());
            }
            if !paths.is_empty() {
                devices.push(DiscoveredWatchdog { index, paths });
            }
        }
        devices.sort_by_key(|device| device.index);
        Ok(devices)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::symlink;
        use tempfile::tempdir;

        fn create_watchdog(sys: &Path, dev: &Path, index: u32, identity: &str) {
            let entry = sys.join(format!("watchdog{index}"));
            fs::create_dir_all(&entry).unwrap();
            fs::write(entry.join("identity"), identity).unwrap();
            File::create(dev.join(format!("watchdog{index}"))).unwrap();
        }

        #[test]
        fn discovers_hardware_in_numeric_order_and_excludes_softdog() {
            let temp = tempdir().unwrap();
            let sys = temp.path().join("sys");
            let dev = temp.path().join("dev");
            fs::create_dir_all(&sys).unwrap();
            fs::create_dir_all(&dev).unwrap();
            create_watchdog(&sys, &dev, 12, "Hardware watchdog");
            create_watchdog(&sys, &dev, 2, "Board WDT");
            create_watchdog(&sys, &dev, 1, "Software Watchdog");

            let found = discover_at(&sys, &dev).unwrap();
            assert_eq!(
                found.iter().map(|item| item.index).collect::<Vec<_>>(),
                [2, 12]
            );
        }

        #[test]
        fn does_not_duplicate_matching_watchdog_alias() {
            let temp = tempdir().unwrap();
            let sys = temp.path().join("sys");
            let dev = temp.path().join("dev");
            fs::create_dir_all(&sys).unwrap();
            fs::create_dir_all(&dev).unwrap();
            create_watchdog(&sys, &dev, 0, "Board WDT");
            symlink("watchdog0", dev.join("watchdog")).unwrap();

            let found = discover_at(&sys, &dev).unwrap();
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].paths, vec![dev.join("watchdog0")]);
        }

        #[test]
        fn missing_sysfs_directory_means_unsupported() {
            let temp = tempdir().unwrap();
            let found = discover_at(&temp.path().join("missing"), temp.path()).unwrap();
            assert!(found.is_empty());
        }

        #[test]
        fn excludes_softdog_identified_by_driver() {
            let temp = tempdir().unwrap();
            let sys = temp.path().join("sys");
            let dev = temp.path().join("dev");
            fs::create_dir_all(&sys).unwrap();
            fs::create_dir_all(&dev).unwrap();
            create_watchdog(&sys, &dev, 0, "Watchdog");
            let device = sys.join("watchdog0/device");
            fs::create_dir_all(&device).unwrap();
            symlink("/sys/bus/platform/drivers/softdog", device.join("driver")).unwrap();

            assert!(discover_at(&sys, &dev).unwrap().is_empty());
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::{Backend, Device, DiscoveredWatchdog};
    use std::io;
    use std::path::Path;

    #[derive(Default)]
    pub struct UnsupportedBackend;

    impl Backend for UnsupportedBackend {
        fn discover(&self) -> io::Result<Vec<DiscoveredWatchdog>> {
            Ok(Vec::new())
        }

        fn open(&self, _path: &Path) -> io::Result<Box<dyn Device>> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "hardware watchdog is unsupported on Windows",
            ))
        }
    }
}

#[derive(Debug, Clone)]
struct DiscoveredWatchdog {
    index: u32,
    paths: Vec<PathBuf>,
}

trait Device: Send {
    fn keep_alive(&mut self) -> io::Result<()>;
    fn timeout(&mut self) -> io::Result<u32>;
    fn disable(&mut self) -> io::Result<()>;
}

trait Backend: Send + Sync {
    fn discover(&self) -> io::Result<Vec<DiscoveredWatchdog>>;
    fn open(&self, path: &std::path::Path) -> io::Result<Box<dyn Device>>;
}

#[derive(Debug, Clone)]
pub struct WatchdogRuntimeStatus {
    pub supported: bool,
    pub running: bool,
    pub reason: Option<String>,
}

#[derive(Default)]
struct SharedState {
    running: bool,
    last_error: Option<String>,
}

enum WorkerCommand {
    Disable(oneshot::Sender<io::Result<()>>),
}

struct RunningWatchdog {
    commands: mpsc::Sender<WorkerCommand>,
    task: JoinHandle<()>,
}

pub struct WatchdogController {
    backend: Arc<dyn Backend>,
    shared: Arc<StdMutex<SharedState>>,
    running: Mutex<Option<RunningWatchdog>>,
}

impl Default for WatchdogController {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchdogController {
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        let backend = Arc::new(platform::LinuxBackend::default());
        #[cfg(windows)]
        let backend = Arc::new(platform::UnsupportedBackend);
        Self::with_backend(backend)
    }

    fn with_backend(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            shared: Arc::new(StdMutex::new(SharedState::default())),
            running: Mutex::new(None),
        }
    }

    pub async fn enable(&self) -> io::Result<()> {
        let mut running = self.running.lock().await;
        if running.is_some() {
            return Ok(());
        }

        let devices = self.backend.discover().map_err(|error| {
            self.record_error(format!("Failed to discover hardware watchdog: {error}"));
            error
        })?;
        if devices.is_empty() {
            let message = "No hardware watchdog device found";
            self.record_error(message.to_string());
            return Err(io::Error::new(io::ErrorKind::NotFound, message));
        }

        let mut open_errors = Vec::new();
        let mut selected = None;
        'devices: for device in devices {
            for path in device.paths {
                match self.backend.open(&path) {
                    Ok(mut handle) => {
                        let initial_error = handle
                            .keep_alive()
                            .err()
                            .map(|error| format!("Watchdog initial keepalive failed: {error}"));
                        selected = Some((path, handle, initial_error));
                        break 'devices;
                    }
                    Err(error) => open_errors.push(format!("{}: {error}", path.display())),
                }
            }
        }

        let Some((path, mut device, initial_error)) = selected else {
            let message = format!(
                "Failed to open a hardware watchdog: {}",
                open_errors.join("; ")
            );
            self.record_error(message.clone());
            return Err(io::Error::new(io::ErrorKind::Other, message));
        };

        let timeout = device.timeout().unwrap_or(30);
        let period = Duration::from_secs(u64::from((timeout / 3).max(1)));
        let (commands, receiver) = mpsc::channel(1);
        let shared = self.shared.clone();
        {
            let mut state = shared.lock().unwrap();
            state.running = initial_error.is_none();
            state.last_error = initial_error;
        }
        let task = tokio::spawn(run_worker(device, receiver, shared, period, path));
        *running = Some(RunningWatchdog { commands, task });
        Ok(())
    }

    pub async fn disable(&self) -> io::Result<()> {
        let mut running = self.running.lock().await;
        let Some(worker) = running.as_mut() else {
            let mut state = self.shared.lock().unwrap();
            state.running = false;
            state.last_error = None;
            return Ok(());
        };

        let (result_tx, result_rx) = oneshot::channel();
        worker
            .commands
            .send(WorkerCommand::Disable(result_tx))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "watchdog worker stopped"))?;
        match result_rx.await {
            Ok(Ok(())) => {
                if let Some(worker) = running.take() {
                    let _ = worker.task.await;
                }
                Ok(())
            }
            Ok(Err(error)) => Err(error),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "watchdog worker stopped",
            )),
        }
    }

    pub async fn status(&self) -> WatchdogRuntimeStatus {
        let discovery = self.backend.discover();
        let supported = discovery.as_ref().is_ok_and(|devices| !devices.is_empty());
        let state = self.shared.lock().unwrap();
        let reason = if let Err(error) = discovery {
            Some(format!("Failed to discover hardware watchdog: {error}"))
        } else if !supported {
            Some("No hardware watchdog device found".to_string())
        } else {
            state.last_error.clone()
        };
        WatchdogRuntimeStatus {
            supported,
            running: state.running,
            reason,
        }
    }

    fn record_error(&self, message: String) {
        let mut state = self.shared.lock().unwrap();
        state.running = false;
        state.last_error = Some(message);
    }
}

async fn run_worker(
    mut device: Box<dyn Device>,
    mut commands: mpsc::Receiver<WorkerCommand>,
    shared: Arc<StdMutex<SharedState>>,
    period: Duration,
    path: PathBuf,
) {
    let mut ticker = tokio::time::interval(period);
    ticker.tick().await;
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                match device.keep_alive() {
                    Ok(()) => {
                        let mut state = shared.lock().unwrap();
                        state.running = true;
                        state.last_error = None;
                    }
                    Err(error) => {
                        let message = format!("Watchdog keepalive failed: {error}");
                        tracing::error!("{} ({})", message, path.display());
                        let mut state = shared.lock().unwrap();
                        state.running = false;
                        state.last_error = Some(message);
                    }
                }
            }
            command = commands.recv() => {
                let Some(WorkerCommand::Disable(result_tx)) = command else {
                    break;
                };
                match device.disable() {
                    Ok(()) => {
                        let mut state = shared.lock().unwrap();
                        state.running = false;
                        state.last_error = None;
                        let _ = result_tx.send(Ok(()));
                        break;
                    }
                    Err(error) => {
                        let message = format!("Hardware watchdog cannot be safely disabled: {error}");
                        tracing::error!("{}; continuing keepalive", message);
                        let mut state = shared.lock().unwrap();
                        state.running = true;
                        state.last_error = Some(message);
                        let _ = result_tx.send(Err(error));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeDevice {
        feeds: Arc<AtomicUsize>,
        feed_results: Arc<StdMutex<VecDeque<io::Result<()>>>>,
        disable_result: Arc<StdMutex<Option<io::Result<()>>>>,
        timeout: u32,
    }

    impl Device for FakeDevice {
        fn keep_alive(&mut self) -> io::Result<()> {
            self.feeds.fetch_add(1, Ordering::SeqCst);
            self.feed_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(()))
        }
        fn timeout(&mut self) -> io::Result<u32> {
            Ok(self.timeout)
        }
        fn disable(&mut self) -> io::Result<()> {
            self.disable_result.lock().unwrap().take().unwrap_or(Ok(()))
        }
    }

    struct FakeBackend {
        feeds: Arc<AtomicUsize>,
        feed_results: Arc<StdMutex<VecDeque<io::Result<()>>>>,
        disable_result: Arc<StdMutex<Option<io::Result<()>>>>,
        opens: Arc<AtomicUsize>,
    }

    impl Backend for FakeBackend {
        fn discover(&self) -> io::Result<Vec<DiscoveredWatchdog>> {
            Ok(vec![
                DiscoveredWatchdog {
                    index: 0,
                    paths: vec![PathBuf::from("/dev/watchdog0")],
                },
                DiscoveredWatchdog {
                    index: 1,
                    paths: vec![PathBuf::from("/dev/watchdog1")],
                },
            ])
        }
        fn open(&self, _path: &Path) -> io::Result<Box<dyn Device>> {
            self.opens.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakeDevice {
                feeds: self.feeds.clone(),
                feed_results: self.feed_results.clone(),
                disable_result: self.disable_result.clone(),
                timeout: 3,
            }))
        }
    }

    fn fake_controller(
        disable_result: io::Result<()>,
    ) -> (WatchdogController, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        fake_controller_with_feeds(disable_result, VecDeque::new())
    }

    fn fake_controller_with_feeds(
        disable_result: io::Result<()>,
        feed_results: VecDeque<io::Result<()>>,
    ) -> (WatchdogController, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let feeds = Arc::new(AtomicUsize::new(0));
        let opens = Arc::new(AtomicUsize::new(0));
        let backend = FakeBackend {
            feeds: feeds.clone(),
            feed_results: Arc::new(StdMutex::new(feed_results)),
            disable_result: Arc::new(StdMutex::new(Some(disable_result))),
            opens: opens.clone(),
        };
        (
            WatchdogController::with_backend(Arc::new(backend)),
            feeds,
            opens,
        )
    }

    #[tokio::test]
    async fn enable_feeds_immediately_and_disable_stops_worker() {
        let (controller, feeds, opens) = fake_controller(Ok(()));
        controller.enable().await.unwrap();
        assert_eq!(opens.load(Ordering::SeqCst), 1);
        assert_eq!(feeds.load(Ordering::SeqCst), 1);
        assert!(controller.status().await.running);
        controller.disable().await.unwrap();
        assert!(!controller.status().await.running);
    }

    #[tokio::test]
    async fn failed_disable_keeps_watchdog_running() {
        let (controller, feeds, _) = fake_controller(Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "nowayout",
        )));
        controller.enable().await.unwrap();
        assert!(controller.disable().await.is_err());
        assert!(controller.status().await.running);
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(feeds.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn initial_feed_failure_retries_the_same_device() {
        let feed_results = VecDeque::from([
            Err(io::Error::new(io::ErrorKind::Other, "temporary failure")),
            Ok(()),
        ]);
        let (controller, feeds, opens) = fake_controller_with_feeds(Ok(()), feed_results);

        controller.enable().await.unwrap();
        assert!(!controller.status().await.running);
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(controller.status().await.running);
        assert_eq!(opens.load(Ordering::SeqCst), 1);
        assert!(feeds.load(Ordering::SeqCst) >= 2);
        controller.disable().await.unwrap();
    }
}
