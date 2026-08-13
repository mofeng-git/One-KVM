use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::error::{AppError, Result};

const IDLE_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const APLAY_QUEUE_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UacPlaybackState {
    Idle,
    Waiting,
    Active,
    Stalled,
}

impl UacPlaybackState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Waiting => "waiting",
            Self::Active => "active",
            Self::Stalled => "stalled",
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Idle,
            1 => Self::Waiting,
            2 => Self::Active,
            _ => Self::Stalled,
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Waiting => 1,
            Self::Active => 2,
            Self::Stalled => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UacPlaybackConfig {
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

impl Default for UacPlaybackConfig {
    fn default() -> Self {
        Self {
            device_name: crate::otg::uac::find_uac_pcm_device()
                .unwrap_or_else(crate::otg::uac::uac_pcm_device),
            sample_rate: 48_000,
            channels: 2,
        }
    }
}

struct PlaybackInner {
    config: UacPlaybackConfig,
    stopped: AtomicBool,
    active_session: Mutex<Option<Arc<SessionShared>>>,
}

struct SessionShared {
    state: AtomicU8,
}

#[derive(Clone)]
pub struct UacPlayback {
    inner: Arc<PlaybackInner>,
}

pub struct UacSession {
    playback: UacPlayback,
    tx: mpsc::Sender<Vec<u8>>,
    shared: Arc<SessionShared>,
}

impl UacPlayback {
    pub fn start(config: UacPlaybackConfig) -> Result<Self> {
        if config.sample_rate != 48_000 || config.channels != 2 {
            return Err(AppError::BadRequest(
                "UAC playback supports only 48000 Hz stereo".to_string(),
            ));
        }

        Ok(Self {
            inner: Arc::new(PlaybackInner {
                config,
                stopped: AtomicBool::new(false),
                active_session: Mutex::new(None),
            }),
        })
    }

    pub fn acquire_session(&self) -> Result<UacSession> {
        let mut active = self.inner.active_session.lock().unwrap();
        if self.inner.stopped.load(Ordering::Acquire) {
            return Err(AppError::ServiceUnavailable(
                "UAC playback is stopping".to_string(),
            ));
        }
        if active.is_some() {
            return Err(AppError::ServiceUnavailable(
                "another UAC microphone session is already active".to_string(),
            ));
        }

        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(APLAY_QUEUE_DEPTH);
        let shared = Arc::new(SessionShared {
            state: AtomicU8::new(UacPlaybackState::Waiting.to_u8()),
        });
        *active = Some(Arc::clone(&shared));
        drop(active);

        let config = self.inner.config.clone();
        let thread_state = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("uac-aplay".into())
            .spawn(move || {
                aplay_loop(&config, &mut rx, &thread_state);
            })
            .map_err(|e| AppError::Internal(format!("Failed to spawn UAC playback thread: {e}")))?;

        Ok(UacSession {
            playback: self.clone(),
            tx,
            shared,
        })
    }

    pub fn stop(&self) {
        if self.inner.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(shared) = self.inner.active_session.lock().unwrap().take() {
            shared
                .state
                .store(UacPlaybackState::Idle.to_u8(), Ordering::Release);
        }
    }
}

impl UacSession {
    pub fn state(&self) -> UacPlaybackState {
        UacPlaybackState::from_u8(self.shared.state.load(Ordering::Acquire))
    }

    /// Returns whether the frame was accepted and the resulting state.
    pub fn try_write(&self, pcm: &[i16]) -> Result<(bool, UacPlaybackState)> {
        if self.playback.inner.stopped.load(Ordering::Acquire) {
            return Err(AppError::ServiceUnavailable(
                "UAC playback has stopped".to_string(),
            ));
        }

        // Convert i16 samples to interleaved S16LE bytes for aplay stdin.
        let mut bytes = Vec::with_capacity(pcm.len() * 2);
        for sample in pcm {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        match self.tx.try_send(bytes) {
            Ok(()) => Ok((true, self.state())),
            Err(mpsc::error::TrySendError::Full(_)) => Ok((false, self.state())),
            Err(mpsc::error::TrySendError::Closed(_)) => Ok((false, UacPlaybackState::Stalled)),
        }
    }
}

impl Drop for UacSession {
    fn drop(&mut self) {
        let mut active = self.playback.inner.active_session.lock().unwrap();
        if active
            .as_ref()
            .is_some_and(|session| Arc::ptr_eq(session, &self.shared))
        {
            *active = None;
        }
        self.shared
            .state
            .store(UacPlaybackState::Idle.to_u8(), Ordering::Release);
    }
}

fn spawn_aplay(config: &UacPlaybackConfig) -> Option<(Child, Box<dyn Write + Send>)> {
    let mut cmd = Command::new("aplay");
    cmd.arg("-D")
        .arg(&config.device_name)
        .arg("-f")
        .arg("S16_LE")
        .arg("-r")
        .arg(config.sample_rate.to_string())
        .arg("-c")
        .arg(config.channels.to_string())
        .arg("--buffer-size=32768")
        .arg("--period-size=1024")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    match cmd.spawn() {
        Ok(mut child) => {
            let stdin = child.stdin.take()?;
            info!("UAC aplay spawned (pid={})", child.id());
            Some((child, Box::new(stdin)))
        }
        Err(error) => {
            warn!("Failed to spawn UAC aplay: {error}");
            None
        }
    }
}

fn aplay_loop(
    config: &UacPlaybackConfig,
    rx: &mut mpsc::Receiver<Vec<u8>>,
    state: &Arc<SessionShared>,
) {
    let mut aplay: Option<(Child, Box<dyn Write + Send>)> = None;
    let mut last_write = Instant::now();
    let mut was_idle = false;

    loop {
        // Poll for data with an idle-aware timeout so a stalled aplay
        // gets closed instead of blocking the worker thread forever.
        let need_timeout = aplay.is_some() && last_write.elapsed() >= IDLE_CLOSE_TIMEOUT;
        let deadline = if need_timeout || aplay.is_none() {
            Some(Instant::now() + Duration::from_millis(200))
        } else {
            None
        };

        let frame = loop {
            match rx.try_recv() {
                Ok(f) => break Some(f),
                Err(mpsc::error::TryRecvError::Disconnected) => break None,
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
            if let Some(dl) = deadline {
                if Instant::now() >= dl {
                    break None;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        };

        match frame {
            Some(f) => {
                last_write = Instant::now();

                // Resuming after idle — force a fresh aplay to avoid
                // reusing a process stuck in an underrun state.
                if was_idle {
                    if let Some((c, s)) = aplay.take() {
                        kill_aplay(c, s);
                    }
                    was_idle = false;
                }

                // Ensure aplay is alive.
                if aplay.is_none() {
                    aplay = spawn_aplay(config);
                }

                if let Some((child, _stdin)) = aplay.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            warn!("UAC aplay exited: {status}");
                            aplay = spawn_aplay(config);
                            if aplay.is_none() {
                                continue;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            warn!("UAC aplay wait error: {error}");
                            aplay = None;
                            continue;
                        }
                    }
                }

                if let Some((_child, stdin)) = aplay.as_mut() {
                    match stdin.write_all(&f) {
                        Ok(()) => {
                            let _ = stdin.flush();
                            state
                                .state
                                .store(UacPlaybackState::Active.to_u8(), Ordering::Release);
                        }
                        Err(error) => {
                            warn!("UAC aplay write failed: {error}");
                            if let Some((c, s)) = aplay.take() {
                                kill_aplay(c, s);
                            }
                            state
                                .state
                                .store(UacPlaybackState::Stalled.to_u8(), Ordering::Release);
                        }
                    }
                }
            }
            None => {
                if let Some((c, s)) = aplay.take() {
                    kill_aplay(c, s);
                }
                was_idle = true;
                state
                    .state
                    .store(UacPlaybackState::Waiting.to_u8(), Ordering::Release);
                if rx.is_closed() {
                    break;
                }
            }
        }
    }

    // Cleanup.
    if let Some((c, s)) = aplay.take() {
        kill_aplay(c, s);
    }
    info!("UAC aplay thread stopped");
}

fn kill_aplay(mut child: Child, stdin: Box<dyn Write + Send>) {
    drop(stdin); // close pipe → EOF for aplay
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permits_only_one_microphone_session() {
        let playback = UacPlayback::start(UacPlaybackConfig::default()).unwrap();
        let first = playback.acquire_session().unwrap();
        assert_eq!(first.state(), UacPlaybackState::Waiting);
        assert!(playback.acquire_session().is_err());

        drop(first);
        assert!(playback.acquire_session().is_ok());
        playback.stop();
    }

    #[test]
    fn stop_rejects_new_and_existing_session_writes() {
        let playback = UacPlayback::start(UacPlaybackConfig::default()).unwrap();
        let session = playback.acquire_session().unwrap();

        playback.stop();

        assert!(session.try_write(&[0, 0]).is_err());
        assert!(playback.acquire_session().is_err());
    }

    #[test]
    fn state_roundtrips_through_u8() {
        for state in [
            UacPlaybackState::Idle,
            UacPlaybackState::Waiting,
            UacPlaybackState::Active,
            UacPlaybackState::Stalled,
        ] {
            assert_eq!(UacPlaybackState::from_u8(state.to_u8()), state);
        }
    }
}
