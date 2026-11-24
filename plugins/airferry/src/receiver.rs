//! Built-in optical file reception from the existing MJPEG frame buffer.
use anyhow::{anyhow, Result};
use one_kvm_airferry_core::{
    pool::{DecoderPool, RecoveryWorker},
    receiver::Receiver,
};
use one_kvm_plugin_sdk::{runtime::Client, Codec};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    sync::{oneshot, Mutex, RwLock},
    task::JoinHandle,
    time::MissedTickBehavior,
};

struct Worker {
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}
#[derive(Default)]
struct Events {
    latest: Value,
    saved: Option<Value>,
}

pub struct AirferryManager {
    output: PathBuf,
    import_images: bool,
    worker: Mutex<Option<Worker>>,
    events: Arc<RwLock<Events>>,
}

#[derive(Serialize)]
pub struct AirferryReceivedFile {
    pub name: String,
    pub display_name: String,

    pub size: u64,

    pub modified_unix_ms: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirferryProgress {
    pub file: String,
    pub received_symbols: u32,
    pub total_symbols: u32,
    pub symbol_size: u32,
    pub bytes_per_second: f64,
    pub decoded_fraction: f64,
    pub meta_confirmed: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirferrySavedFile {
    pub filename: String,

    pub bytes: u64,
    pub receive_seconds: f64,
    pub bytes_per_second: f64,
}

/// Public event fields. Native recovery details stay inside the receiver.

#[derive(Debug, Deserialize, Serialize)]
pub struct AirferryEvent {
    pub state: String,
    pub reason: Option<String>,
    pub jpeg_ms: Option<f64>,
    pub qr_ms: Option<f64>,

    pub decoded_jpegs: Option<u64>,
    pub progress: Option<AirferryProgress>,
    pub received: Option<u32>,
    pub total: Option<u32>,
    pub filename: Option<String>,
    pub path: Option<String>,

    pub bytes: Option<u64>,
    pub sha256: Option<String>,
    pub receive_seconds: Option<f64>,
    pub bytes_per_second: Option<f64>,
}

#[derive(Serialize)]
pub struct AirferryStatus {
    pub available: bool,
    pub running: bool,
    pub eligible: bool,
    pub reason: String,
    pub event: Option<AirferryEvent>,
    pub last_saved: Option<AirferrySavedFile>,
    pub files: Vec<AirferryReceivedFile>,
}

impl AirferryManager {
    pub fn new(output: PathBuf, import_images: bool) -> Arc<Self> {
        Arc::new(Self {
            output,
            import_images,
            worker: Mutex::new(None),
            events: Arc::new(RwLock::new(Events::default())),
        })
    }

    pub async fn status(&self, eligible: bool, reason: &str) -> Result<AirferryStatus> {
        let running = self
            .worker
            .lock()
            .await
            .as_ref()
            .is_some_and(|worker| !worker.task.is_finished());
        let (latest, saved) = {
            let events = self.events.read().await;
            (events.latest.clone(), events.saved.clone())
        };
        let files = self.files().await?;
        Ok(AirferryStatus {
            available: true,
            running,
            eligible,
            reason: reason.into(),
            event: serde_json::from_value(latest).map_err(internal)?,
            last_saved: saved
                .map(serde_json::from_value)
                .transpose()
                .map_err(internal)?,
            files,
        })
    }

    pub async fn start(&self, stream: Client) -> Result<()> {
        let mut worker = self.worker.lock().await;
        if worker
            .as_ref()
            .is_some_and(|worker| !worker.task.is_finished())
        {
            return Ok(());
        }
        if let Some(previous) = worker.take() {
            let _ = previous.task.await;
        }
        let output = self.output.clone();
        let receiver =
            tokio::task::spawn_blocking(move || Receiver::new(&output, 64 * 1024 * 1024))
                .await
                .map_err(internal)?
                .map_err(internal)?;
        let pool = DecoderPool::new(3).map_err(internal)?;
        let recovery = RecoveryWorker::new(receiver).map_err(internal)?;
        *self.events.write().await = Events {
            latest: json!({"state":"starting"}),
            saved: None,
        };
        let (stop, stopped) = oneshot::channel();
        let events = self.events.clone();
        let import_images = self.import_images;
        let output = self.output.clone();
        let task = tokio::spawn(async move {
            let mut pool = pool;
            let result = receive(&stream, stopped, &events, &mut pool, &recovery).await;
            if let Ok(Some(name)) = &result {
                if import_images {
                    let imported = async {
                        if !is_received_name(name) || name.contains(['/', '\\']) {
                            return Err(anyhow!("Invalid received filename"));
                        }
                        let file = tokio::fs::File::open(output.join(name)).await?;
                        stream.put_image(name, file).await
                    }
                    .await;
                    if let Err(error) = imported {
                        tracing::warn!(
                            "File received but could not be imported into virtual media: {error}"
                        );
                        events.write().await.latest = json!({"state":"saved", "reason": format!("Virtual media import failed: {error}")});
                    }
                }
            }
            // Wait for every native decoder to release its buffers before running=false.
            let _ = tokio::task::spawn_blocking(move || {
                pool.shutdown();
                recovery.shutdown();
            })
            .await;
            if let Err(error) = result {
                tracing::warn!("AirFerry: {}", error);
                events.write().await.latest = json!({"state":"error", "reason":error.to_string()});
            }
        });
        *worker = Some(Worker { stop, task });
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut worker = self.worker.lock().await;
        if let Some(worker) = worker.take() {
            let _ = worker.stop.send(());
            worker.task.await.map_err(internal)?;
        }
        Ok(())
    }

    pub async fn files(&self) -> Result<Vec<AirferryReceivedFile>> {
        let mut files = Vec::new();
        let mut directory = match tokio::fs::read_dir(self.output.clone()).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(files),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = directory.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_received_name(&name) {
                continue;
            }
            let meta = match tokio::fs::symlink_metadata(entry.path()).await {
                Ok(meta) if meta.is_file() => meta,
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            let display_name = display_name(&name);
            let modified = meta
                .modified()
                .ok()
                .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|v| v.as_millis() as u64)
                .unwrap_or(0);
            files.push(AirferryReceivedFile {
                name,
                display_name,
                size: meta.len(),
                modified_unix_ms: modified,
            });
        }
        files.sort_by(|a, b| b.modified_unix_ms.cmp(&a.modified_unix_ms));
        files.truncate(100);
        Ok(files)
    }
}

fn is_received_name(name: &str) -> bool {
    name.len() > 33
        && name.as_bytes().get(32) == Some(&b'-')
        && name.as_bytes()[..32].iter().all(u8::is_ascii_hexdigit)
}

fn internal(error: impl std::fmt::Display) -> anyhow::Error {
    anyhow!("{error}")
}

pub async fn eligibility(stream: &Client) -> (bool, &'static str) {
    let Ok(info) = stream.video_info().await else {
        return (false, "capture_unavailable");
    };
    if info.mode != Codec::Mjpeg {
        return (false, "mjpeg_required");
    }
    if !info.capture_format.as_deref().is_some_and(|format| {
        format.eq_ignore_ascii_case("MJPEG") || format.eq_ignore_ascii_case("JPEG")
    }) {
        return (false, "native_jpeg_required");
    }
    if !info.ready {
        return (false, "capture_unavailable");
    }
    (true, "ready")
}

async fn receive(
    stream: &Client,
    mut stop: oneshot::Receiver<()>,
    events: &RwLock<Events>,
    pool: &mut DecoderPool,
    recovery: &RecoveryWorker,
) -> Result<Option<String>> {
    let mut gate_timer = tokio::time::interval(Duration::from_secs(1));
    gate_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut frames: Option<tokio::sync::mpsc::Receiver<one_kvm_plugin_sdk::Frame>> = None;
    let mut allowed = false;
    let mut reason = "";
    let mut epoch = 0;
    let mut sequence = 0;

    let mut last_cost = None;
    let mut decoded_jpegs = 0u64;
    loop {
        tokio::select! {
            _ = &mut stop => {
                events.write().await.latest = json!({"state":"stopped"});
                return Ok(None);
            }
            _ = gate_timer.tick() => {
                let (eligible, next_reason) = eligibility(&stream).await;
                if eligible != allowed || reason != next_reason {
                    epoch += 1;
                    allowed = eligible;
                    reason = next_reason;
                    events.write().await.latest = json!({"state":if allowed {"waiting"} else {"paused"}, "reason":reason});
                }
                if allowed && frames.is_none() {
                    match stream.subscribe_video(Codec::Mjpeg).await {
                        Ok(receiver) => frames = Some(receiver),
                        Err(error) => {
                            allowed = false;
                            events.write().await.latest = json!({"state":"error", "reason":error.to_string()});
                        }
                    }
                } else if !allowed {
                    frames = None;
                }
                if allowed {
                    if let Some((jpeg_ms, qr_ms)) = last_cost {
                        let recovered = recovery.ingest(Vec::new(), sequence, true).await.map_err(internal)?;
                        events.write().await.latest = json!({"state":"scanning", "jpeg_ms":jpeg_ms, "qr_ms":qr_ms,
                            "decoded_jpegs":decoded_jpegs, "progress":recovered.progress});
                    }
                }
            }
            frame = async {
                match frames.as_mut() {
                    Some(receiver) => receiver.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                let Some(frame) = frame else {
                    frames = None;
                    allowed = false;
                    epoch += 1;
                    continue;
                };
                if !pool.has_capacity() { continue }
                sequence += 1;
                pool.submit(frame.data, sequence, epoch).map_err(internal)?;
            }
            Some(decoded) = pool.receive() => {
                if !allowed || decoded.epoch != epoch { continue }
                let (payloads, jpeg_ms, qr_ms) = match decoded.result {
                    Ok(result) => result,
                    Err(error) => {
                        tracing::debug!("AirFerry rejected frame: {}", error);
                        continue;
                    }
                };
                last_cost = Some((jpeg_ms, qr_ms));
                decoded_jpegs += 1;
                if payloads.is_empty() { continue }
                let recovered = recovery.ingest(payloads, decoded.sequence, false).await.map_err(internal)?;
                let mut state = events.write().await;
                for update in recovered.updates {
                    if update["state"] == "saved" {
                        state.saved = Some(update.clone());
                        let name = update["path"].as_str().and_then(|path| {
                            std::path::Path::new(path).file_name()?.to_str().map(str::to_owned)
                        });
                        state.latest = update;
                        return Ok(name);
                    }
                    state.latest = update;
                }
            }
        }
    }
}

pub fn display_name(name: &str) -> String {
    if is_received_name(name) {
        name[33..].into()
    } else {
        name.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receiver_events_preserve_the_public_response_fields() {
        // Samples match the native recovery worker's scanning, segmented and saved events.
        for source in [
            json!({"state":"scanning", "jpeg_ms":1.0, "qr_ms":2.0, "decoded_jpegs":4,
                "progress":{"file":"file.bin", "received_symbols":3, "total_symbols":8,
                    "symbol_size":1400, "bytes_per_second":2800.0,
                    "decoded_fraction":0.375, "meta_confirmed":true}}),
            json!({"state":"segment_saved", "received":1, "total":2}),
            json!({"state":"saved", "path":"/data/file.bin", "filename":"file.bin",
                "bytes":4096, "sha256":"checksum", "receive_seconds":2.0, "bytes_per_second":2048.0}),
            json!({"state":"error", "reason":"capture unavailable"}),
        ] {
            let event: AirferryEvent = serde_json::from_value(source.clone()).unwrap();
            let serialized = serde_json::to_value(event).unwrap();
            for (key, value) in source.as_object().unwrap() {
                assert_eq!(&serialized[key], value, "{key}");
            }
        }
        assert!(serde_json::from_value::<Option<AirferryEvent>>(Value::Null)
            .unwrap()
            .is_none());
    }
}
