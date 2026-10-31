//! HDMI 链路状态探针。
//!
//! MacroSilicon 系列（MS2109 / MS2130 / MS2130S …）的固件里有一个寄存器记录 HDMI
//! 链路锁定状态，可以通过 HID 厂商接口（/dev/hidraw*）实时读取，一次约 6ms。
//!
//! **不同芯片/固件差异极大**：寄存器地址和取值都可能不同（见过 0/1、也见过 0x0E）。
//! 所以这里支持三种模式（存于 /etc/one-kvm/appearance.json 的 `signal` 字段）：
//!
//! - `auto`（默认）：MacroSilicon 全家 + 0x1F87，只认 0/1，读到别的值就判定
//!   "这张卡不支持"并停用彩条（宁可不显示，也不能把真实画面盖成彩条）；
//! - `off`：总是当作"有信号"，永不出彩条；
//! - `custom`：用户自填 VID / PID / 寄存器地址 / "有信号"时的读数，适配逆向出来的卡。
//!
//! 设置页有"读取一次"，可以直接看到当前卡的原始读数。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const DEFAULT_REGISTER: u16 = 0x1F87;
const DEFAULT_LOCKED_VALUE: u8 = 1;
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

/// 按当前设置读一次寄存器（设置页"读取一次"用）。
pub fn read_once() -> Result<(String, u8), String> {
    imp::read_once()
}

/// 连续读多次，用于判断这个寄存器是否稳定（设置页"读取 10 次"用）。
pub fn read_sample(count: usize) -> Result<(String, Vec<u8>), String> {
    imp::read_sample(count)
}

/// 解析后的探针配置
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProbeConfig {
    pub mode: ProbeMode,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub register: u16,
    pub locked_value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ProbeMode {
    Auto,
    Off,
    Custom,
}

fn parse_hex_u16(value: &str) -> Option<u16> {
    let trimmed = value.trim().trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(trimmed, 16).ok()
}

pub(crate) fn current_config() -> ProbeConfig {
    let appearance = crate::appearance::current();
    let signal = appearance.signal.clone().unwrap_or_default();
    let mode = match signal.preset.as_deref().map(str::trim) {
        Some("off") => ProbeMode::Off,
        Some("custom") => ProbeMode::Custom,
        _ => ProbeMode::Auto,
    };
    let (default_vid, default_pid) = (Some(0x345F), None);
    ProbeConfig {
        mode,
        vid: match mode {
            ProbeMode::Custom => signal.vid.as_deref().and_then(parse_hex_u16),
            _ => default_vid,
        },
        pid: signal.pid.as_deref().and_then(parse_hex_u16).or(default_pid),
        register: signal
            .register
            .as_deref()
            .and_then(parse_hex_u16)
            .unwrap_or(DEFAULT_REGISTER),
        locked_value: signal
            .locked_value
            .map(|v| v as u8)
            .unwrap_or(DEFAULT_LOCKED_VALUE),
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    const REPORT_LEN: usize = 9;
    const CMD_READ_REG: u8 = 0xB5;
    /// custom 模式下允许出现的"异常值"上限（auto 模式更严格）
    const INVALID_LIMIT: u32 = 10;

    fn hid_ioc(nr: u8, len: usize) -> libc::c_ulong {
        // _IOWR('H', nr, len)
        let dir: u32 = 0b11;
        ((dir << 30) | ((len as u32) << 16) | ((b'H' as u32) << 8) | nr as u32) as libc::c_ulong
    }

    fn find_node(cfg: &ProbeConfig) -> Option<String> {
        for i in 0..16 {
            let uevent = format!("/sys/class/hidraw/hidraw{i}/device/uevent");
            let Ok(text) = std::fs::read_to_string(&uevent) else {
                continue;
            };
            let upper = text.to_uppercase();
            let Some(pos) = upper.find("HID_ID=0003:") else {
                continue;
            };
            let ids = &upper[pos + "HID_ID=0003:".len()..];
            let mut parts = ids.split(':');
            let vid = parts.next().and_then(parse_hex_u16);
            let pid = parts.next().and_then(parse_hex_u16);
            // 未指定 VID 时只匹配 MacroSilicon 家族（345F / 534D），避免误绑到键盘等其它 HID
            let vid_match = match cfg.vid {
                Some(want) => vid == Some(want),
                None => matches!(vid, Some(0x345F) | Some(0x534D)),
            };
            let pid_match = cfg.pid.map(|want| pid == Some(want)).unwrap_or(true);
            if vid_match && pid_match {
                return Some(format!("/dev/hidraw{i}"));
            }
        }
        None
    }

    fn read_register(fd: &File, register: u16) -> std::io::Result<u8> {
        let mut set = [0u8; REPORT_LEN];
        set[1] = CMD_READ_REG;
        set[2] = (register >> 8) as u8;
        set[3] = (register & 0xFF) as u8;
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
        Ok(get[4])
    }

    pub(super) fn read_sample(count: usize) -> Result<(String, Vec<u8>), String> {
        let cfg = current_config();
        if cfg.mode == ProbeMode::Off {
            return Err("当前设置为“关闭检测”".to_string());
        }
        let node = find_node(&cfg)
            .ok_or_else(|| format!("没有找到匹配的 HID 设备（vid={:04X?}）", cfg.vid))?;
        let file = File::options()
            .read(true)
            .write(true)
            .open(&node)
            .map_err(|e| format!("打开 {node} 失败: {e}"))?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count.clamp(1, 50) {
            match read_register(&file, cfg.register) {
                Ok(v) => values.push(v),
                Err(e) => return Err(format!("读寄存器 0x{:04X} 失败: {e}", cfg.register)),
            }
            std::thread::sleep(Duration::from_millis(60));
        }
        Ok((node, values))
    }

    pub(super) fn read_once() -> Result<(String, u8), String> {
        let cfg = current_config();
        if cfg.mode == ProbeMode::Off {
            return Err("当前设置为“关闭检测”".to_string());
        }
        let node = find_node(&cfg)
            .ok_or_else(|| format!("没有找到匹配的 HID 设备（vid={:04X?}）", cfg.vid))?;
        let file = File::options()
            .read(true)
            .write(true)
            .open(&node)
            .map_err(|e| format!("打开 {node} 失败: {e}"))?;
        let value = read_register(&file, cfg.register)
            .map_err(|e| format!("读寄存器 0x{:04X} 失败: {e}", cfg.register))?;
        Ok((node, value))
    }

    pub(super) fn spawn(locked: Arc<AtomicBool>) -> Option<()> {
        let cfg = current_config();
        if cfg.mode == ProbeMode::Off {
            locked.store(true, Ordering::Relaxed);
            return None;
        }
        let node = find_node(&cfg)?;
        std::thread::Builder::new()
            .name("hdmi-signal-probe".into())
            .spawn(move || {
                let debug = std::env::var("ONE_KVM_SIGNAL_DEBUG").as_deref() == Ok("1");
                let mut fd: Option<File> = None;
                let mut misses = 0u32;
                let mut invalid = 0u32;
                let mut polls = 0u64;
                let mut config = current_config();
                loop {
                    // 设置可以被随时修改：变了就重新解析（含节点重新匹配）
                    let latest = current_config();
                    if latest != config {
                        config = latest;
                        fd = None;
                        invalid = 0;
                        if config.mode == ProbeMode::Off {
                            locked.store(true, Ordering::Relaxed);
                        }
                    }
                    if config.mode == ProbeMode::Off {
                        locked.store(true, Ordering::Relaxed);
                        std::thread::sleep(POLL_INTERVAL);
                        continue;
                    }
                    if fd.is_none() {
                        fd = File::options()
                            .read(true)
                            .write(true)
                            .open(&node)
                            .ok();
                    }
                    if let Some(file) = fd.as_ref() {
                        match read_register(file, config.register) {
                            Ok(value) => {
                                misses = 0;
                                polls += 1;
                                if debug && polls % 25 == 0 {
                                    tracing::info!(
                                        "[SIGNAL] node={} raw=0x{:02X} expect_locked=0x{:02X} mode={:?}",
                                        node,
                                        value,
                                        config.locked_value,
                                        config.mode
                                    );
                                }
                                if config.mode == ProbeMode::Custom {
                                    // 自定义模式：等于设定值就是"有信号"
                                    locked.store(value == config.locked_value, Ordering::Relaxed);
                                    invalid = 0;
                                } else {
                                    // auto 模式：只认 0/1
                                    match value {
                                        0 => {
                                            invalid = 0;
                                            locked.store(false, Ordering::Relaxed);
                                        }
                                        1 => {
                                            invalid = 0;
                                            locked.store(true, Ordering::Relaxed);
                                        }
                                        other => {
                                            invalid += 1;
                                            if invalid == INVALID_LIMIT {
                                                tracing::warn!(
                                                    "采集卡 {} 的 0x{:04X} 读到 0x{:02X}（非 0/1），\
                                                     判定不支持链路标志，停用无信号彩条；\
                                                     可在 设置→外观→无信号检测 里改成自定义",
                                                    node,
                                                    config.register,
                                                    other
                                                );
                                                locked.store(true, Ordering::Relaxed);
                                                // 不退出线程：用户可以随时改成自定义
                                                invalid = 0;
                                                std::thread::sleep(Duration::from_secs(2));
                                            }
                                        }
                                    }
                                }
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

    pub(super) fn read_once() -> Result<(String, u8), String> {
        Err("仅支持 Linux".to_string())
    }

    pub(super) fn read_sample(_count: usize) -> Result<(String, Vec<u8>), String> {
        Err("仅支持 Linux".to_string())
    }
}
