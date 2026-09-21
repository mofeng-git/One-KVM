use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alsa::pcm::{Access, Format, Frames, HwParams, State};
use alsa::{Direction, ValueOr, PCM};
use tracing::{info, warn};

use crate::error::{AppError, Result};

const RETRY_BACKOFF: Duration = Duration::from_secs(1);
const PERIOD_FRAMES: Frames = 1_024;
// Request the same compatibility buffer as the known-working ALSA player.
// The gadget driver may negotiate a smaller buffer; always use its result.
const BUFFER_FRAMES: Frames = 32_768;
const IDLE_REOPEN_TIMEOUT: Duration = Duration::from_secs(5);
const SINK_STALL_TIMEOUT: Duration = Duration::from_millis(200);

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
    active_session: Mutex<Option<Arc<Mutex<SessionRuntime>>>>,
}

enum SessionSink {
    Closed {
        retry_at: Option<Instant>,
    },
    Probing {
        pcm: PlaybackPcm,
        stalled: bool,
    },
    Active {
        pcm: PlaybackPcm,
        last_progress: Instant,
    },
}

impl SessionSink {
    fn state(&self) -> UacPlaybackState {
        match self {
            Self::Closed { retry_at: None } => UacPlaybackState::Waiting,
            Self::Closed { retry_at: Some(_) } => UacPlaybackState::Stalled,
            Self::Probing { stalled: false, .. } => UacPlaybackState::Waiting,
            Self::Probing { stalled: true, .. } => UacPlaybackState::Stalled,
            Self::Active { .. } => UacPlaybackState::Active,
        }
    }
}

struct SessionRuntime {
    sink: SessionSink,
    last_frame: Option<Instant>,
}

impl SessionRuntime {
    fn new() -> Self {
        Self {
            sink: SessionSink::Closed { retry_at: None },
            last_frame: None,
        }
    }

    fn state(&self) -> UacPlaybackState {
        self.sink.state()
    }

    fn close(&mut self) {
        self.sink = SessionSink::Closed { retry_at: None };
        self.last_frame = None;
    }

    /// Advance playback only when a WebSocket frame arrives. All ALSA handles
    /// are non-blocking, so a slow or absent USB host drops the current frame
    /// instead of occupying a worker thread or accumulating stale speech.
    fn write(&mut self, config: &UacPlaybackConfig, samples: &[i16]) -> bool {
        // Reopen on resume without an idle timer thread. stop()/session drop
        // still close the PCM synchronously, even when no frames arrive.
        if self
            .last_frame
            .is_some_and(|last| last.elapsed() >= IDLE_REOPEN_TIMEOUT)
        {
            self.close();
        }
        self.last_frame = Some(Instant::now());
        let sink = std::mem::replace(&mut self.sink, SessionSink::Closed { retry_at: None });
        let (next_sink, accepted) = drive_sink(sink, config, samples);
        self.sink = next_sink;
        accepted
    }
}

#[derive(Clone)]
pub struct UacPlayback {
    inner: Arc<PlaybackInner>,
}

pub struct UacSession {
    playback: UacPlayback,
    runtime: Arc<Mutex<SessionRuntime>>,
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

        let runtime = Arc::new(Mutex::new(SessionRuntime::new()));
        *active = Some(Arc::clone(&runtime));
        Ok(UacSession {
            playback: self.clone(),
            runtime,
        })
    }

    /// Stop accepting frames and synchronously close an active ALSA handle.
    /// This guarantees configfs may rebuild the UAC function after this call.
    pub fn stop(&self) {
        if self.inner.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        let runtime = self.inner.active_session.lock().unwrap().take();
        if let Some(runtime) = runtime {
            runtime.lock().unwrap().close();
        }
    }
}

impl UacSession {
    pub fn state(&self) -> UacPlaybackState {
        self.runtime.lock().unwrap().state()
    }

    /// Returns whether the frame was accepted and the resulting target state.
    pub fn try_write(&self, pcm: &[i16]) -> Result<(bool, UacPlaybackState)> {
        let channels = self.playback.inner.config.channels as usize;
        if pcm.is_empty() || !pcm.len().is_multiple_of(channels) {
            return Err(AppError::BadRequest(
                "UAC PCM must contain complete stereo frames".to_string(),
            ));
        }
        if self.playback.inner.stopped.load(Ordering::Acquire) {
            return Err(AppError::ServiceUnavailable(
                "UAC playback has stopped".to_string(),
            ));
        }

        let mut runtime = self.runtime.lock().unwrap();
        if self.playback.inner.stopped.load(Ordering::Acquire) {
            runtime.close();
            return Err(AppError::ServiceUnavailable(
                "UAC playback has stopped".to_string(),
            ));
        }
        let accepted = runtime.write(&self.playback.inner.config, pcm);
        Ok((accepted, runtime.state()))
    }
}

impl Drop for UacSession {
    fn drop(&mut self) {
        let mut active = self.playback.inner.active_session.lock().unwrap();
        if active
            .as_ref()
            .is_some_and(|session| Arc::ptr_eq(session, &self.runtime))
        {
            *active = None;
        }
        drop(active);
        self.runtime.lock().unwrap().close();
    }
}

fn drive_sink(
    sink: SessionSink,
    config: &UacPlaybackConfig,
    samples: &[i16],
) -> (SessionSink, bool) {
    match sink {
        SessionSink::Closed { retry_at } => {
            if retry_at.is_some_and(|deadline| Instant::now() < deadline) {
                return (SessionSink::Closed { retry_at }, false);
            }

            match open_pcm(config).and_then(|mut pcm| {
                pcm.prime_with_silence(config.channels as usize)?;
                Ok(pcm)
            }) {
                Ok(pcm) => drive_probe(pcm, false, config, samples),
                Err(error) => {
                    warn!("Failed to open UAC playback device; retrying later: {error}");
                    (
                        SessionSink::Closed {
                            retry_at: Some(Instant::now() + RETRY_BACKOFF),
                        },
                        false,
                    )
                }
            }
        }
        SessionSink::Probing { pcm, stalled } => drive_probe(pcm, stalled, config, samples),
        SessionSink::Active { pcm, last_progress } => {
            drive_active(pcm, last_progress, config, samples)
        }
    }
}

fn drive_probe(
    mut pcm: PlaybackPcm,
    stalled: bool,
    config: &UacPlaybackConfig,
    samples: &[i16],
) -> (SessionSink, bool) {
    match pcm.consumption_progress() {
        Ok(false) => (SessionSink::Probing { pcm, stalled }, false),
        Ok(true) => {
            // Keep the stream that has just started consuming. Dropping and
            // preparing it here creates another startup/underrun window.
            info!("UAC target started consuming microphone audio");
            drive_active(pcm, Instant::now(), config, samples)
        }
        Err(error) => recover_sink(pcm, config, error),
    }
}

fn drive_active(
    mut pcm: PlaybackPcm,
    last_progress: Instant,
    config: &UacPlaybackConfig,
    samples: &[i16],
) -> (SessionSink, bool) {
    let last_progress = match pcm.consumption_progress() {
        Ok(true) => Instant::now(),
        Ok(false) => last_progress,
        Err(error) => return recover_sink(pcm, config, error),
    };
    if last_progress.elapsed() >= SINK_STALL_TIMEOUT {
        // Discard queued speech before probing an unavailable host again.
        if let Err(error) = pcm.reset_and_prime(config.channels as usize) {
            warn!("Failed to reset stalled UAC playback: {error}");
            return retry_later();
        }
        info!("UAC target stopped consuming audio; waiting for playback activity");
        return (SessionSink::Probing { pcm, stalled: true }, false);
    }

    match pcm.write_samples(samples, config.channels as usize) {
        Ok(accepted) => (SessionSink::Active { pcm, last_progress }, accepted),
        Err(error) => recover_sink(pcm, config, error),
    }
}

fn recover_sink(
    mut pcm: PlaybackPcm,
    config: &UacPlaybackConfig,
    error: alsa::Error,
) -> (SessionSink, bool) {
    match error.errno() {
        libc::EAGAIN | libc::EINTR => (SessionSink::Probing { pcm, stalled: true }, false),
        libc::EPIPE | libc::ESTRPIPE => {
            // prepare restarts after XRUN/suspend without snd_pcm_recover's
            // potentially unbounded resume loop. Start again with silence,
            // and require fresh consumption before reporting Active.
            match pcm.reset_and_prime(config.channels as usize) {
                Ok(()) => (SessionSink::Probing { pcm, stalled: true }, false),
                Err(error) => {
                    warn!("Failed to recover UAC playback: {error}");
                    retry_later()
                }
            }
        }
        _ => {
            warn!("UAC playback failed; reopening later: {error}");
            retry_later()
        }
    }
}

fn retry_later() -> (SessionSink, bool) {
    (
        SessionSink::Closed {
            retry_at: Some(Instant::now() + RETRY_BACKOFF),
        },
        false,
    )
}

fn open_pcm(config: &UacPlaybackConfig) -> Result<PlaybackPcm> {
    let pcm = PCM::new(&config.device_name, Direction::Playback, true).map_err(|error| {
        AppError::AudioError(format!(
            "Failed to open UAC device {}: {error}",
            config.device_name
        ))
    })?;
    {
        let params = HwParams::any(&pcm)
            .map_err(|error| AppError::AudioError(format!("UAC HwParams failed: {error}")))?;
        params
            .set_channels(config.channels as u32)
            .and_then(|_| params.set_rate(config.sample_rate, ValueOr::Nearest))
            .and_then(|_| params.set_format(Format::s16()))
            .and_then(|_| params.set_access(Access::RWInterleaved))
            .and_then(|_| params.set_period_size_near(PERIOD_FRAMES, ValueOr::Nearest))
            .and_then(|_| params.set_buffer_size_near(BUFFER_FRAMES))
            .and_then(|_| pcm.hw_params(&params))
            .map_err(|error| {
                AppError::AudioError(format!("Failed to configure UAC playback: {error}"))
            })?;
    }

    let (buffer_frames, period_frames) = pcm.get_params().map_err(|error| {
        AppError::AudioError(format!("Failed to read UAC PCM parameters: {error}"))
    })?;
    {
        let params = pcm.sw_params_current().map_err(|error| {
            AppError::AudioError(format!("Failed to read UAC SwParams: {error}"))
        })?;
        params
            .set_start_threshold(buffer_frames as Frames)
            .and_then(|_| params.set_stop_threshold(buffer_frames as Frames))
            .and_then(|_| params.set_avail_min(period_frames as Frames))
            .and_then(|_| pcm.sw_params(&params))
            .map_err(|error| {
                AppError::AudioError(format!("Failed to configure UAC SwParams: {error}"))
            })?;
    }
    pcm.prepare().map_err(|error| {
        AppError::AudioError(format!("Failed to prepare UAC playback: {error}"))
    })?;
    info!(
        "UAC playback opened on {} (buffer={} frames, period={} frames)",
        config.device_name, buffer_frames, period_frames
    );
    Ok(PlaybackPcm {
        pcm,
        buffer_frames: buffer_frames as Frames,
        period_frames: period_frames as Frames,
        submitted_frames: 0,
        consumed_frames: 0,
    })
}

struct PlaybackPcm {
    pcm: PCM,
    buffer_frames: Frames,
    period_frames: Frames,
    submitted_frames: u64,
    consumed_frames: u64,
}

impl PlaybackPcm {
    fn consumption_progress(&mut self) -> std::result::Result<bool, alsa::Error> {
        // avail synchronizes the hardware pointer. Successful writes alone
        // only show that the ring buffer has room, not that USB is consuming.
        let available = self.pcm.avail()?;
        match self.pcm.state() {
            State::XRun => return Err(alsa::Error::new("UAC PCM state", libc::EPIPE)),
            State::Suspended => return Err(alsa::Error::new("UAC PCM state", libc::ESTRPIPE)),
            State::Disconnected => return Err(alsa::Error::new("UAC PCM state", libc::ENODEV)),
            State::Running => {}
            _ => return Ok(false),
        }
        let consumed = consumed_frames(self.submitted_frames, self.buffer_frames, available);
        let progressed = consumed > self.consumed_frames;
        self.consumed_frames = consumed;
        Ok(progressed)
    }

    fn write_samples(
        &mut self,
        samples: &[i16],
        channels: usize,
    ) -> std::result::Result<bool, alsa::Error> {
        let io = self.pcm.io_i16()?;
        let written = write_frames(samples, channels, self.period_frames as usize, |chunk| {
            let written = io.writei(chunk)?;
            self.submitted_frames += written as u64;
            Ok(written)
        })?;
        Ok(written == samples.len() / channels)
    }

    fn prime_with_silence(&mut self, channels: usize) -> Result<()> {
        // Use the negotiated capacity, not BUFFER_FRAMES. A near request is
        // often clamped by u_audio's DMA buffer limit.
        let silence = vec![0i16; self.buffer_frames as usize * channels];
        let complete = self
            .write_samples(&silence, channels)
            .map_err(|error| AppError::AudioError(format!("Failed to prime UAC PCM: {error}")))?;
        if !complete {
            return Err(AppError::AudioError(
                "UAC PCM priming was interrupted".into(),
            ));
        }
        // Most hardware starts automatically at the threshold. Some PCM
        // plugins remain Prepared despite accepting the complete prefill.
        // Start explicitly only after priming, and never restart a running PCM.
        if self.pcm.state() == State::Prepared {
            self.pcm.start().map_err(|error| {
                AppError::AudioError(format!("Failed to start primed UAC PCM: {error}"))
            })?;
        }
        Ok(())
    }

    fn reset_and_prime(&mut self, channels: usize) -> Result<()> {
        self.pcm
            .drop()
            .and_then(|_| self.pcm.prepare())
            .map_err(|error| AppError::AudioError(format!("Failed to reset UAC PCM: {error}")))?;
        self.submitted_frames = 0;
        self.consumed_frames = 0;
        self.prime_with_silence(channels)
    }
}

fn consumed_frames(submitted: u64, buffer: Frames, available: Frames) -> u64 {
    let queued = (buffer - available.clamp(0, buffer)) as u64;
    submitted.saturating_sub(queued)
}

/// Bound each write to one negotiated period and advance by actual frames,
/// including short writes. Never wait for space or retain stale audio.
fn write_frames(
    samples: &[i16],
    channels: usize,
    period_frames: usize,
    mut write: impl FnMut(&[i16]) -> std::result::Result<usize, alsa::Error>,
) -> std::result::Result<usize, alsa::Error> {
    let total_frames = samples.len() / channels;
    let mut offset = 0;
    while offset < total_frames {
        let end = (offset + period_frames).min(total_frames);
        match write(&samples[offset * channels..end * channels]) {
            Ok(0) => break,
            Ok(written) => offset += written,
            Err(error) if matches!(error.errno(), libc::EAGAIN | libc::EINTR) => break,
            Err(error) => return Err(error),
        }
    }
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_short_frames_without_skipping_stereo_samples() {
        let samples: Vec<i16> = (0..24).collect();
        let mut received = Vec::new();
        let written = write_frames(&samples, 2, 4, |chunk| {
            assert!(chunk.len() <= 8);
            // Simulate a device accepting only one frame per write.
            received.extend_from_slice(&chunk[..2]);
            Ok(1)
        })
        .unwrap();
        assert_eq!(written, 12);
        assert_eq!(received, samples);
    }

    #[test]
    fn full_device_stops_writing_without_waiting_or_claiming_whole_packet() {
        let mut calls = 0;
        let written = write_frames(&[0; 24], 2, 4, |_| {
            calls += 1;
            if calls == 1 {
                Ok(2)
            } else {
                Err(alsa::Error::new("test write", libc::EAGAIN))
            }
        })
        .unwrap();
        assert_eq!(written, 2);
        assert_eq!(calls, 2);
    }

    #[test]
    fn writing_into_free_space_is_not_host_consumption() {
        // A partially filled or full buffer can exist without any USB I/O.
        assert_eq!(consumed_frames(1024, 4096, 3072), 0);
        assert_eq!(consumed_frames(4096, 4096, 0), 0);
        // Consuming a period, followed by filling it again, preserves progress.
        assert_eq!(consumed_frames(4096, 4096, 1024), 1024);
        assert_eq!(consumed_frames(5120, 4096, 0), 1024);
    }

    fn null_config() -> UacPlaybackConfig {
        UacPlaybackConfig {
            device_name: "null".into(),
            sample_rate: 48_000,
            channels: 2,
        }
    }

    #[test]
    fn native_pcm_uses_negotiated_start_threshold_and_recovers_to_probing() {
        // ALSA's null plugin exercises real libasound configuration and I/O
        // without requiring a USB controller. It cannot verify DWC3 behavior.
        let config = null_config();
        let mut pcm = open_pcm(&config).unwrap();
        assert_eq!(
            pcm.pcm
                .sw_params_current()
                .unwrap()
                .get_start_threshold()
                .unwrap(),
            pcm.buffer_frames
        );
        pcm.prime_with_silence(2).unwrap();
        assert!(pcm.consumption_progress().unwrap());
        let (sink, accepted) =
            recover_sink(pcm, &config, alsa::Error::new("test xrun", libc::EPIPE));
        assert!(!accepted);
        assert!(matches!(sink, SessionSink::Probing { stalled: true, .. }));
    }

    #[test]
    fn stop_closes_an_open_native_pcm_before_returning() {
        let playback = UacPlayback::start(null_config()).unwrap();
        let session = playback.acquire_session().unwrap();
        session.try_write(&[0; 2048]).unwrap();
        assert!(!matches!(
            session.runtime.lock().unwrap().sink,
            SessionSink::Closed { .. }
        ));
        playback.stop();
        assert!(matches!(
            session.runtime.lock().unwrap().sink,
            SessionSink::Closed { retry_at: None }
        ));
    }

    #[test]
    fn idle_resume_reopens_instead_of_reusing_previous_sink() {
        let config = null_config();
        let mut runtime = SessionRuntime::new();
        runtime.sink = SessionSink::Closed {
            retry_at: Some(Instant::now() + Duration::from_secs(60)),
        };
        runtime.last_frame = Some(Instant::now() - IDLE_REOPEN_TIMEOUT);
        runtime.write(&config, &[0; 2048]);
        assert!(!matches!(runtime.sink, SessionSink::Closed { .. }));
    }

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
    fn rejects_incomplete_stereo_frames_before_opening_alsa() {
        let playback = UacPlayback::start(UacPlaybackConfig::default()).unwrap();
        let session = playback.acquire_session().unwrap();

        assert!(session.try_write(&[0]).is_err());
        assert_eq!(session.state(), UacPlaybackState::Waiting);
    }

    #[test]
    fn closed_sink_state_reflects_retry_backoff() {
        assert_eq!(
            SessionSink::Closed { retry_at: None }.state(),
            UacPlaybackState::Waiting
        );
        assert_eq!(
            SessionSink::Closed {
                retry_at: Some(Instant::now())
            }
            .state(),
            UacPlaybackState::Stalled
        );
    }
}
