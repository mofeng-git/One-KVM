use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::mpsc;

use super::frame_mailbox::FrameMailbox;
use super::EncodedVideoFrame;
use crate::video::frame::VideoFrame;

pub(super) type EncodedBatch = Arc<[Arc<EncodedVideoFrame>]>;
pub(super) type FrameMailboxHandle = Arc<Mutex<Option<Arc<FrameMailbox<Arc<VideoFrame>>>>>>;

#[derive(Default)]
pub(super) struct DeliveryStats {
    pub sent_frames: AtomicU64,
    pub send_time_ns: AtomicU64,
    pub send_errors: AtomicU64,
}

pub(super) struct VideoSubscriber {
    pub sender: mpsc::Sender<EncodedBatch>,
    pub needs_keyframe: AtomicBool,
    pub recovery_requested: AtomicBool,
}

pub(super) struct VideoFrameReservation {
    pub subscriber: Arc<VideoSubscriber>,
    pub permit: mpsc::OwnedPermit<EncodedBatch>,
}

pub struct EncodedVideoFrameReceiver {
    receiver: mpsc::Receiver<EncodedBatch>,
    pending: VecDeque<Arc<EncodedVideoFrame>>,
    mailbox: FrameMailboxHandle,
    stats: Arc<DeliveryStats>,
}

impl EncodedVideoFrameReceiver {
    pub(super) fn new(
        receiver: mpsc::Receiver<EncodedBatch>,
        mailbox: FrameMailboxHandle,
        stats: Arc<DeliveryStats>,
    ) -> Self {
        Self {
            receiver,
            pending: VecDeque::new(),
            mailbox,
            stats,
        }
    }

    fn notify_capacity(&self) {
        if let Some(mailbox) = self.mailbox.lock().as_ref() {
            mailbox.notify();
        }
    }

    pub async fn recv(&mut self) -> Option<Arc<EncodedVideoFrame>> {
        loop {
            if let Some(frame) = self.pending.pop_front() {
                return Some(frame);
            }
            let batch = self.receiver.recv().await?;
            self.pending.extend(batch.iter().cloned());
            self.notify_capacity();
        }
    }

    pub fn try_recv(&mut self) -> Result<Arc<EncodedVideoFrame>, mpsc::error::TryRecvError> {
        loop {
            if let Some(frame) = self.pending.pop_front() {
                return Ok(frame);
            }
            let batch = self.receiver.try_recv()?;
            self.pending.extend(batch.iter().cloned());
            self.notify_capacity();
        }
    }

    pub(crate) fn record_send(&self, elapsed: Duration, succeeded: bool) {
        self.stats
            .send_time_ns
            .fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);
        if succeeded {
            self.stats.sent_frames.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.send_errors.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Drop for EncodedVideoFrameReceiver {
    fn drop(&mut self) {
        self.receiver.close();
        self.notify_capacity();
    }
}
