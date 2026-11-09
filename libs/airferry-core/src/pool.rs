//! Bounded JPEG/QR workers. JPEG owners are shared with capture; no frame bytes are copied.
use crate::decoder::Decoder;
use anyhow::{Context, Result};
use bytes::Bytes;
use std::{sync::mpsc, thread};
use tokio::sync::mpsc as async_mpsc;

fn lower_worker_priority() {
    #[cfg(target_os = "linux")]
    unsafe {
        // Linux scheduling priority belongs to a thread. Do not renice the KVM.
        libc::setpriority(
            libc::PRIO_PROCESS,
            libc::syscall(libc::SYS_gettid) as u32,
            15,
        );
    }
}

struct Job {
    jpeg: Bytes,
    sequence: u64,
    epoch: u64,
}

pub struct Decoded {
    worker: usize,
    pub sequence: u64,
    pub epoch: u64,
    pub result: Result<(Vec<Vec<u8>>, f64, f64)>,
}

pub struct DecoderPool {
    workers: Vec<mpsc::SyncSender<Job>>,
    threads: Vec<thread::JoinHandle<()>>,
    results: async_mpsc::Receiver<Decoded>,
    busy: Vec<bool>,
}

impl DecoderPool {
    pub fn new(count: usize) -> Result<Self> {
        anyhow::ensure!((1..=3).contains(&count), "Expected 1 to 3 decoder workers");
        let (results, receiver) = async_mpsc::channel(count);
        let mut pool = Self {
            workers: Vec::with_capacity(count),
            threads: Vec::with_capacity(count),
            results: receiver,
            busy: vec![false; count],
        };
        for worker in 0..count {
            let (sender, jobs) = mpsc::sync_channel::<Job>(1);
            let results = results.clone();
            let thread = thread::Builder::new()
                .name(format!("airferry-qr-{worker}"))
                .spawn(move || {
                    lower_worker_priority();
                    let mut decoder = Decoder::new();
                    while let Ok(job) = jobs.recv() {
                        let result = match &mut decoder {
                            Ok(decoder) => decoder.decode(&job.jpeg),
                            Err(error) => Err(anyhow::anyhow!(error.to_string())),
                        };
                        if results
                            .blocking_send(Decoded {
                                worker,
                                sequence: job.sequence,
                                epoch: job.epoch,
                                result,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .context("Cannot start AirFerry decoder thread")?;
            pool.workers.push(sender);
            pool.threads.push(thread);
        }
        Ok(pool)
    }

    pub fn has_capacity(&self) -> bool {
        self.busy.iter().any(|busy| !busy)
    }

    /// At most one frame per worker, including decoded results awaiting consumption.
    pub fn submit(&mut self, jpeg: Bytes, sequence: u64, epoch: u64) -> Result<bool> {
        let Some(worker) = self.busy.iter().position(|busy| !busy) else {
            return Ok(false);
        };
        self.workers[worker]
            .try_send(Job {
                jpeg,
                sequence,
                epoch,
            })
            .context("Decoder worker stopped")?;
        self.busy[worker] = true;
        Ok(true)
    }

    pub async fn receive(&mut self) -> Option<Decoded> {
        let result = self.results.recv().await?;
        self.busy[result.worker] = false;
        Some(result)
    }

    /// Call from a blocking task, after dropping pending decode results.
    pub fn shutdown(mut self) {
        self.results.close();
        self.workers.clear();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Drop for DecoderPool {
    fn drop(&mut self) {
        self.results.close();
        self.workers.clear();
    }
}

pub struct Recovered {
    pub updates: Vec<serde_json::Value>,
    pub progress: Option<serde_json::Value>,
}

struct RecoveryJob {
    payloads: Vec<Vec<u8>>,
    sequence: u64,
    report: bool,
    reply: tokio::sync::oneshot::Sender<Recovered>,
}

/// Keep recovery and disk I/O off both the async executor and its shared blocking pool.
pub struct RecoveryWorker {
    jobs: Option<mpsc::SyncSender<RecoveryJob>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl RecoveryWorker {
    pub fn new(mut receiver: crate::receiver::Receiver) -> Result<Self> {
        let (sender, jobs) = mpsc::sync_channel::<RecoveryJob>(1);
        let thread = thread::Builder::new().name("airferry-recover".into()).spawn(move || {
            lower_worker_priority();
            while let Ok(job) = jobs.recv() {
                let mut updates = Vec::new();
                for payload in job.payloads {
                    match receiver.ingest_sequenced(&payload, job.sequence) {
                        Ok(Some(value)) => updates.push(value),
                        Ok(None) => (),
                        Err(error) => updates.push(serde_json::json!({"state":"payload_rejected", "reason":error.to_string()})),
                    }
                }
                let progress = job.report.then(|| receiver.progress());
                let _ = job.reply.send(Recovered { updates, progress });
            }
        }).context("Cannot start AirFerry recovery thread")?;
        Ok(Self {
            jobs: Some(sender),
            thread: Some(thread),
        })
    }

    pub async fn ingest(
        &self,
        payloads: Vec<Vec<u8>>,
        sequence: u64,
        report: bool,
    ) -> Result<Recovered> {
        let (reply, response) = tokio::sync::oneshot::channel();
        self.jobs
            .as_ref()
            .context("Recovery worker stopped")?
            .try_send(RecoveryJob {
                payloads,
                sequence,
                report,
                reply,
            })
            .context("Recovery worker is busy or stopped")?;
        response.await.context("Recovery worker stopped")
    }

    pub fn shutdown(mut self) {
        self.jobs.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for RecoveryWorker {
    fn drop(&mut self) {
        self.jobs.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn completed_frames_remain_bounded_until_consumed() {
        let mut pool = DecoderPool::new(3).unwrap();
        for sequence in 0..3 {
            assert!(pool
                .submit(Bytes::from_static(b"invalid jpeg"), sequence, 7)
                .unwrap());
        }
        assert!(!pool.has_capacity());
        assert!(!pool.submit(Bytes::new(), 4, 7).unwrap());
        for _ in 0..3 {
            let decoded = tokio::time::timeout(std::time::Duration::from_secs(2), pool.receive())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(decoded.epoch, 7);
            assert!(decoded.result.is_err());
        }
        assert!(pool.has_capacity());
        pool.shutdown();
    }
}
