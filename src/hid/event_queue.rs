//! One bounded FIFO for all HID events. Overflow/delivery failure is a reset barrier.
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{mpsc, Mutex, Notify};

struct Shared {
    admission: Mutex<()>,
    failed_epoch: AtomicU64,
    released_epoch: AtomicU64,
    wake: Notify,
}

pub(super) struct Sender<T> {
    tx: mpsc::Sender<T>,
    shared: Arc<Shared>,
    timeout: Duration,
}

pub(super) struct Receiver<T> {
    rx: mpsc::Receiver<T>,
    shared: Arc<Shared>,
    resetting_epoch: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SendError {
    Closed,
    Full,
    Recovering,
}

pub(super) enum Work<T> {
    Event(T),
    Reset,
}

pub(super) fn channel<T>(capacity: usize, timeout: Duration) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel(capacity);
    let shared = Arc::new(Shared {
        admission: Mutex::new(()),
        failed_epoch: AtomicU64::new(0),
        released_epoch: AtomicU64::new(0),
        wake: Notify::new(),
    });
    (
        Sender {
            tx,
            shared: shared.clone(),
            timeout,
        },
        Receiver {
            rx,
            shared,
            resetting_epoch: 0,
        },
    )
}

impl Shared {
    fn recovering(&self) -> bool {
        self.failed_epoch.load(Ordering::Acquire) != self.released_epoch.load(Ordering::Acquire)
    }
    fn invalidate(&self) {
        self.failed_epoch.fetch_add(1, Ordering::AcqRel);
        self.wake.notify_one();
    }
}

// A cancelled Up is just as unsafe as a timed-out Up. Arm before waiting for the
// admission mutex, because cancellation there must also request neutral release.
struct AdmissionGuard {
    shared: Arc<Shared>,
    armed: bool,
}
impl Drop for AdmissionGuard {
    fn drop(&mut self) {
        if self.armed {
            self.shared.invalidate();
        }
    }
}

impl<T> Sender<T> {
    pub async fn send(&self, event: T) -> Result<(), SendError> {
        let mut cancellation = AdmissionGuard {
            shared: self.shared.clone(),
            armed: true,
        };
        // A sender waiting for capacity cannot be bypassed by a newer movement.
        let _admission = self.shared.admission.lock().await;
        let result = if self.shared.recovering() {
            Err(SendError::Recovering)
        } else {
            match tokio::time::timeout(self.timeout, self.tx.send(event)).await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(_)) => Err(SendError::Closed),
                Err(_) => {
                    self.shared.invalidate();
                    Err(SendError::Full)
                }
            }
        };
        cancellation.armed = false;
        result
    }
}

impl<T> Receiver<T> {
    /// Synthetic worker input must obey the same recovery barrier.
    pub fn recovering(&self) -> bool {
        self.shared.recovering()
    }

    pub async fn next(&mut self) -> Option<Work<T>> {
        loop {
            if self.shared.recovering() {
                let _admission = self.shared.admission.lock().await;
                while self.rx.try_recv().is_ok() {}
                self.resetting_epoch = self.shared.failed_epoch.load(Ordering::Acquire);
                return Some(Work::Reset);
            }
            tokio::select! {
                biased;
                _ = self.shared.wake.notified() => {},
                event = self.rx.recv() => {
                    let event = event?;
                    // An overflow may have happened concurrently with dequeue.
                    if !self.shared.recovering() {
                        return Some(Work::Event(event));
                    }
                }
            }
        }
    }

    pub async fn delivery_failed(&mut self) {
        let _admission = self.shared.admission.lock().await;
        self.shared.invalidate();
    }

    pub async fn reset_completed(&mut self, success: bool) {
        if success {
            let _admission = self.shared.admission.lock().await;
            // A cancelled sender can invalidate without taking admission. Release
            // only the generation covered by this reset, never a newer failure.
            self.shared
                .released_epoch
                .store(self.resetting_epoch, Ordering::Release);
        } else {
            // Keep admission closed, with bounded retry load while a device is absent.
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn event<T>(rx: &mut Receiver<T>) -> T {
        match rx.next().await.unwrap() {
            Work::Event(v) => v,
            Work::Reset => panic!("unexpected reset"),
        }
    }

    #[tokio::test]
    async fn saturated_move_cannot_overtake_queued_down() {
        let (tx, mut rx) = channel(2, Duration::from_secs(1));
        tx.send("position A").await.unwrap();
        tx.send("down").await.unwrap();
        let pending = tx.send("position B");
        tokio::pin!(pending);
        assert!(tokio::time::timeout(Duration::from_millis(1), &mut pending)
            .await
            .is_err());
        assert_eq!(event(&mut rx).await, "position A");
        pending.await.unwrap();
        assert_eq!(event(&mut rx).await, "down");
        assert_eq!(event(&mut rx).await, "position B");
    }

    #[tokio::test]
    async fn overflow_discards_batch_and_rejects_new_input_until_reset() {
        let (tx, mut rx) = channel(1, Duration::from_millis(1));
        tx.send("down").await.unwrap();
        assert_eq!(tx.send("up").await, Err(SendError::Full));
        assert_eq!(tx.send("new down").await, Err(SendError::Recovering));
        assert!(matches!(rx.next().await, Some(Work::Reset)));
        rx.reset_completed(true).await;
        tx.send("fresh move").await.unwrap();
        assert_eq!(event(&mut rx).await, "fresh move");
    }

    #[tokio::test]
    async fn failed_delivery_invalidates_backlog_and_failed_reset_keeps_gate_closed() {
        let (tx, mut rx) = channel(4, Duration::from_secs(1));
        for e in ["move", "down", "up"] {
            tx.send(e).await.unwrap();
        }
        assert_eq!(event(&mut rx).await, "move");
        rx.delivery_failed().await;
        assert!(rx.recovering());
        assert!(matches!(rx.next().await, Some(Work::Reset)));
        let started = std::time::Instant::now();
        rx.reset_completed(false).await;
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert_eq!(tx.send("new down").await, Err(SendError::Recovering));
        assert!(matches!(rx.next().await, Some(Work::Reset)));
        rx.reset_completed(true).await;
        tx.send("fresh down").await.unwrap();
        assert_eq!(event(&mut rx).await, "fresh down");
    }

    #[tokio::test]
    async fn empty_failed_batch_still_wakes_reset() {
        let (tx, mut rx) = channel::<u8>(1, Duration::from_secs(1));
        rx.delivery_failed().await;
        assert!(rx.recovering());
        assert!(matches!(rx.next().await, Some(Work::Reset)));
        rx.reset_completed(true).await;
        drop(tx);
        assert!(rx.next().await.is_none());
    }

    #[tokio::test]
    async fn closed_queue_is_an_error() {
        let (tx, rx) = channel(1, Duration::from_secs(1));
        drop(rx);
        assert_eq!(tx.send(1).await, Err(SendError::Closed));
    }

    #[tokio::test]
    async fn cancelled_capacity_wait_requests_release_barrier() {
        let (tx, mut rx) = channel(1, Duration::from_secs(1));
        tx.send(1).await.unwrap();
        {
            let pending = tx.send(2);
            tokio::pin!(pending);
            assert!(tokio::time::timeout(Duration::from_millis(1), &mut pending)
                .await
                .is_err());
            // This pending event could be Up after an already delivered Down.
        }
        assert_eq!(tx.send(3).await, Err(SendError::Recovering));
        assert!(matches!(rx.next().await, Some(Work::Reset)));
        rx.reset_completed(true).await;
        tx.send(3).await.unwrap();
        assert_eq!(event(&mut rx).await, 3);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reset_completion_racing_admission_never_discards_accepted_event() {
        for _ in 0..100 {
            let (tx, mut rx) = channel(1, Duration::from_secs(1));
            rx.delivery_failed().await;
            assert!(matches!(rx.next().await, Some(Work::Reset)));
            let tx = Arc::new(tx);
            let sender = tx.clone();
            let admitted = tokio::spawn(async move { sender.send(7).await });
            rx.reset_completed(true).await;
            match admitted.await.unwrap() {
                Ok(()) => assert_eq!(event(&mut rx).await, 7),
                Err(SendError::Recovering) => {
                    tx.send(9).await.unwrap();
                    assert_eq!(event(&mut rx).await, 9);
                }
                result => panic!("unexpected admission result: {result:?}"),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blocked_sender_is_not_bypassed_by_newer_move() {
        let (tx, mut rx) = channel(1, Duration::from_secs(1));
        let tx = Arc::new(tx);
        tx.send("first").await.unwrap();
        let blocked = tx.send("down");
        tokio::pin!(blocked);
        assert!(tokio::time::timeout(Duration::from_millis(1), &mut blocked)
            .await
            .is_err());
        let sender = tx.clone();
        let newer = tokio::spawn(async move { sender.send("newer move").await });
        assert_eq!(event(&mut rx).await, "first");
        blocked.await.unwrap();
        assert_eq!(event(&mut rx).await, "down");
        newer.await.unwrap().unwrap();
        assert_eq!(event(&mut rx).await, "newer move");
    }

    #[tokio::test]
    async fn cancellation_waiting_for_admission_also_requires_release() {
        let (tx, mut rx) = channel(1, Duration::from_secs(1));
        let admission = tx.shared.admission.lock().await;
        {
            let pending_up = tx.send("up");
            tokio::pin!(pending_up);
            assert!(
                tokio::time::timeout(Duration::from_millis(1), &mut pending_up)
                    .await
                    .is_err()
            );
        }
        drop(admission);
        assert_eq!(tx.send("new down").await, Err(SendError::Recovering));
        assert!(matches!(rx.next().await, Some(Work::Reset)));
        rx.reset_completed(true).await;
        tx.send("fresh").await.unwrap();
        assert_eq!(event(&mut rx).await, "fresh");
    }

    #[tokio::test]
    async fn cancellation_during_reset_cannot_be_erased_by_older_reset() {
        let (tx, mut rx) = channel(1, Duration::from_secs(1));
        rx.delivery_failed().await;
        assert!(rx.recovering());
        assert!(matches!(rx.next().await, Some(Work::Reset)));
        // Worker is now completing neutral release for epoch 1. A sender waiting
        // for admission is cancelled during that operation, creating epoch 2.
        let admission = tx.shared.admission.lock().await;
        {
            let pending_up = tx.send("up");
            tokio::pin!(pending_up);
            assert!(
                tokio::time::timeout(Duration::from_millis(1), &mut pending_up)
                    .await
                    .is_err()
            );
        }
        drop(admission);
        rx.reset_completed(true).await;
        assert_eq!(tx.send("new down").await, Err(SendError::Recovering));
        assert!(matches!(rx.next().await, Some(Work::Reset)));
        rx.reset_completed(true).await;
        tx.send("fresh").await.unwrap();
        assert_eq!(event(&mut rx).await, "fresh");
    }
}
