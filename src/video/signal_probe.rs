//! HDMI 链路状态探针。
//!
//! MS2130 的固件在寄存器 `0x1F87` 里维护链路锁定标志（0=无信号，1=已锁定），
//! 该寄存器可以通过 HID 厂商接口（interface 4 → /dev/hidrawN）实时读取，
//! 读一次约 6ms，比看画面黑不黑可靠得多——画面全黑也是一种有效信号。
//!
//! 找不到设备或读取失败时一律返回「已锁定」，保证不影响原有行为。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const REG_HDMI_LOCK: u16 = 0x1F87;
const CMD_READ_REG: u8 = 0xB5;
const REPORT_LEN: usize = 9;
const POLL_INTERVAL: Duration = Duration::from_millis(200);

pub struct SignalProbe {
    locked: Arc<AtomicBool>,
}

impl SignalProbe {
    /// 启动后台轮询线程；平台不支持或找不到采集卡时返回 None。
    pub fn start() -> Option<Arc<Self>> {
        let locked = Arc::new(AtomicBool::new(true));
        imp::spawn(locked.clone())?;
        Some(Arc::new(Self { locked }))
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }
}

static PROBE: OnceLock<Option<Arc<SignalProbe>>> = OnceLock::new();

/// 全局探针（首次调用时初始化）。
pub fn probe() -> Option<&'static Arc<SignalProbe>> {
    PROBE.get_or_init(SignalProbe::start).as_ref()
}

/// HDMI 是否处于锁定状态。探针不可用时返回 true（保持原有行为）。
pub fn hdmi_locked() -> bool {
    // 调试用：强制按「无信号」处理，便于验证彩条渲染
    static FORCED: OnceLock<bool> = OnceLock::new();
    if *FORCED.get_or_init(|| {
        std::env::var("ONE_KVM_FORCE_BARS")
            .map(|v| v.trim() == "1")
            .unwrap_or(false)
    }) {
        return false;
    }
    match probe() {
        Some(p) => p.is_locked(),
        None => true,
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    fn hid_ioc(nr: u8, len: usize) -> libc::c_ulong {
        // _IOWR('H', nr, len)
        let dir: u32 = 0b11;
        ((dir << 30) | ((len as u32) << 16) | ((b'H' as u32) << 8) | nr as u32) as libc::c_ulong
    }

    fn find_node() -> Option<String> {
        for i in 0..16 {
            let uevent = format!("/sys/class/hidraw/hidraw{i}/device/uevent");
            let Ok(text) = std::fs::read_to_string(&uevent) else {
                continue;
            };
            if text.to_uppercase().contains("HID_ID=0003:0000345F:00002130") {
                return Some(format!("/dev/hidraw{i}"));
            }
        }
        None
    }

    fn read_lock(fd: &File) -> std::io::Result<bool> {
        let mut set = [0u8; REPORT_LEN];
        set[1] = CMD_READ_REG;
        set[2] = (REG_HDMI_LOCK >> 8) as u8;
        set[3] = (REG_HDMI_LOCK & 0xFF) as u8;
        // 厂商集合在 Linux 下用报告号 0，读回值在 buf[4]
        let ret = unsafe { libc::ioctl(fd.as_raw_fd(), hid_ioc(0x06, REPORT_LEN), set.as_ptr()) };
        if ret < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut get = [0u8; REPORT_LEN];
        let ret = unsafe {
            libc::ioctl(
                fd.as_raw_fd(),
                hid_ioc(0x07, REPORT_LEN),
                get.as_mut_ptr(),
            )
        };
        if ret < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(get[4] != 0)
    }

    pub(super) fn spawn(locked: Arc<AtomicBool>) -> Option<()> {
        let node = find_node()?;
        std::thread::Builder::new()
            .name("hdmi-signal-probe".into())
            .spawn(move || {
                let mut fd: Option<File> = None;
                let mut misses = 0u32;
                loop {
                    if fd.is_none() {
                        fd = File::options().read(true).write(true).open(&node).ok();
                    }
                    if let Some(file) = fd.as_ref() {
                        match read_lock(file) {
                            Ok(v) => {
                                locked.store(v, Ordering::Relaxed);
                                misses = 0;
                            }
                            Err(_) => {
                                misses += 1;
                                if misses > 5 {
                                    fd = None;
                                    misses = 0;
                                }
                            }
                        }
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
            })
            .ok()?;
        Some(())
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::*;

    pub(super) fn spawn(_locked: Arc<AtomicBool>) -> Option<()> {
        None
    }
}
