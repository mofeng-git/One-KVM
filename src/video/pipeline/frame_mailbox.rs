use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct FrameWaitTimes {
    pub(super) frame: Duration,
    pub(super) output: Duration,
}

impl FrameWaitTimes {
    fn record(&mut self, pending: bool, elapsed: Duration) {
        if pending {
            self.output += elapsed;
        } else {
            self.frame += elapsed;
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PublishResult {
    Published,
    Replaced,
    Rejected,
}

struct FrameState<Frame> {
    pending: Option<Frame>,
    last_sequence: Option<u64>,
    closed: bool,
}

pub(super) struct FrameMailbox<Frame> {
    state: Mutex<FrameState<Frame>>,
    available: Condvar,
}

impl<Frame> FrameMailbox<Frame> {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(FrameState {
                pending: None,
                last_sequence: None,
                closed: false,
            }),
            available: Condvar::new(),
        }
    }

    pub(super) fn publish(&self, sequence: u64, frame: Frame) -> PublishResult {
        let mut state = self.state.lock().expect("frame mailbox lock poisoned");
        if state.closed || state.last_sequence.is_some_and(|last| sequence <= last) {
            return PublishResult::Rejected;
        }
        state.last_sequence = Some(sequence);
        let outcome = if state.pending.replace(frame).is_some() {
            PublishResult::Replaced
        } else {
            PublishResult::Published
        };
        self.available.notify_one();
        outcome
    }

    #[cfg(test)]
    pub(super) fn receive(&self) -> Option<Frame> {
        self.receive_when(|| Some(())).map(|(frame, ())| frame)
    }

    #[cfg(test)]
    pub(super) fn receive_when<Ready>(
        &self,
        ready: impl FnMut() -> Option<Ready>,
    ) -> Option<(Frame, Ready)> {
        self.receive_when_timed(ready, false)
            .map(|(frame, ready, _)| (frame, ready))
    }

    pub(super) fn receive_when_timed<Ready>(
        &self,
        mut ready: impl FnMut() -> Option<Ready>,
        measure: bool,
    ) -> Option<(Frame, Ready, FrameWaitTimes)> {
        let mut waits = FrameWaitTimes::default();
        let mut state = self.state.lock().expect("frame mailbox lock poisoned");
        loop {
            if state.closed {
                return None;
            }
            if state.pending.is_some() {
                if let Some(ready) = ready() {
                    return state.pending.take().map(|frame| (frame, ready, waits));
                }
            }
            let pending = state.pending.is_some();
            let wait_started = measure.then(Instant::now);
            state = self
                .available
                .wait(state)
                .expect("frame mailbox lock poisoned");
            if let Some(started) = wait_started {
                waits.record(pending, started.elapsed());
            }
        }
    }

    pub(super) fn notify(&self) {
        let _state = self.state.lock().expect("frame mailbox lock poisoned");
        self.available.notify_one();
    }

    pub(super) fn has_pending(&self) -> bool {
        self.state
            .lock()
            .expect("frame mailbox lock poisoned")
            .pending
            .is_some()
    }

    pub(super) fn close(&self) {
        let mut state = self.state.lock().expect("frame mailbox lock poisoned");
        state.closed = true;
        state.pending.take();
        self.available.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    #[test]
    fn pending_frame_is_replaced_by_latest() {
        let mailbox = FrameMailbox::new();
        assert_eq!(mailbox.publish(1, "first"), PublishResult::Published);
        assert!(mailbox.has_pending());
        assert_eq!(mailbox.publish(2, "latest"), PublishResult::Replaced);
        assert_eq!(mailbox.receive(), Some("latest"));
        assert!(!mailbox.has_pending());
    }

    #[test]
    fn consumed_sequence_still_rejects_old_and_duplicate_frames() {
        let mailbox = FrameMailbox::new();
        assert_eq!(mailbox.publish(10, "current"), PublishResult::Published);
        assert_eq!(mailbox.receive(), Some("current"));
        assert_eq!(mailbox.publish(9, "old"), PublishResult::Rejected);
        assert_eq!(mailbox.publish(10, "duplicate"), PublishResult::Rejected);
        assert!(!mailbox.has_pending());
        assert_eq!(mailbox.publish(11, "next"), PublishResult::Published);
        assert_eq!(mailbox.receive(), Some("next"));
    }

    #[test]
    fn close_discards_pending_and_rejects_future_frames() {
        let mailbox = FrameMailbox::new();
        mailbox.publish(1, "pending");
        mailbox.close();
        assert!(!mailbox.has_pending());
        assert_eq!(mailbox.receive(), None);
        assert_eq!(mailbox.publish(2, "late"), PublishResult::Rejected);
    }

    #[test]
    fn receiver_is_woken_by_publish() {
        for sequence in 0..64 {
            let mailbox = Arc::new(FrameMailbox::new());
            let receiver = mailbox.clone();
            let (ready_sender, ready_receiver) = mpsc::channel();
            let (result_sender, result_receiver) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                ready_sender.send(()).unwrap();
                result_sender.send(receiver.receive()).unwrap();
            });
            ready_receiver.recv().unwrap();
            mailbox.publish(sequence, sequence);
            assert_eq!(
                result_receiver
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap(),
                Some(sequence)
            );
            worker.join().unwrap();
        }
    }

    #[test]
    fn close_wakes_idle_receiver() {
        for _ in 0..64 {
            let mailbox = Arc::new(FrameMailbox::<u64>::new());
            let receiver = mailbox.clone();
            let (ready_sender, ready_receiver) = mpsc::channel();
            let (result_sender, result_receiver) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                ready_sender.send(()).unwrap();
                result_sender.send(receiver.receive()).unwrap();
            });
            ready_receiver.recv().unwrap();
            mailbox.close();
            assert_eq!(
                result_receiver
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap(),
                None
            );
            worker.join().unwrap();
        }
    }

    #[test]
    fn wait_times_separate_missing_input_from_full_output() {
        let mut waits = FrameWaitTimes::default();
        waits.record(false, Duration::from_millis(3));
        waits.record(true, Duration::from_millis(7));
        waits.record(false, Duration::from_millis(2));
        assert_eq!(waits.frame, Duration::from_millis(5));
        assert_eq!(waits.output, Duration::from_millis(7));
    }

    #[test]
    fn available_frame_has_no_wait_time() {
        let mailbox = FrameMailbox::new();
        mailbox.publish(1, "ready");
        let result = mailbox.receive_when_timed(|| Some(42), true);
        assert_eq!(result, Some(("ready", 42, FrameWaitTimes::default())));
    }

    #[test]
    fn timed_output_wait_preserves_latest_frame_and_can_be_disabled() {
        use std::sync::atomic::{AtomicBool, Ordering};

        for measure in [false, true] {
            let mailbox = Arc::new(FrameMailbox::new());
            mailbox.publish(1, "initial");
            let ready = Arc::new(AtomicBool::new(false));
            let receiver_mailbox = mailbox.clone();
            let receiver_ready = ready.clone();
            let (waiting_sender, waiting_receiver) = mpsc::channel();
            let (result_sender, result_receiver) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let mut reported = false;
                let result = receiver_mailbox.receive_when_timed(
                    || {
                        if receiver_ready.load(Ordering::Acquire) {
                            Some(42)
                        } else {
                            if !reported {
                                waiting_sender.send(()).unwrap();
                                reported = true;
                            }
                            None
                        }
                    },
                    measure,
                );
                result_sender.send(result).unwrap();
            });
            waiting_receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            mailbox.publish(2, "latest");
            ready.store(true, Ordering::Release);
            mailbox.notify();
            let (frame, permit, waits) = result_receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap();
            assert_eq!((frame, permit), ("latest", 42));
            assert_eq!(waits.frame, Duration::ZERO);
            if measure {
                assert!(waits.output > Duration::ZERO);
            } else {
                assert_eq!(waits, FrameWaitTimes::default());
            }
            worker.join().unwrap();
        }
    }

    #[test]
    fn timed_receiver_exits_when_closed_without_output_capacity() {
        let mailbox = Arc::new(FrameMailbox::new());
        mailbox.publish(1, "pending");
        let receiver = mailbox.clone();
        let (waiting_sender, waiting_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut reported = false;
            let result = receiver.receive_when_timed(
                || {
                    if !reported {
                        waiting_sender.send(()).unwrap();
                        reported = true;
                    }
                    None::<()>
                },
                true,
            );
            result_sender.send(result).unwrap();
        });
        waiting_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        mailbox.close();
        assert_eq!(
            result_receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            None
        );
        worker.join().unwrap();
    }
}
