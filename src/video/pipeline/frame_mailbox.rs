use std::sync::{Condvar, Mutex};

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

    pub(super) fn receive_when<Ready>(
        &self,
        mut ready: impl FnMut() -> Option<Ready>,
    ) -> Option<(Frame, Ready)> {
        self.wait_until_ready(|pending| {
            pending.as_ref()?;
            let output = ready()?;
            pending.take().map(|frame| (frame, output))
        })
    }

    #[cfg(any(test, all(target_os = "linux", feature = "rkmpp")))]
    pub(super) fn wait_for_event<Ready>(
        &self,
        mut ready: impl FnMut() -> Option<Ready>,
    ) -> Option<Ready> {
        self.wait_until_ready(|_| ready())
    }

    fn wait_until_ready<Ready>(
        &self,
        mut ready: impl FnMut(&mut Option<Frame>) -> Option<Ready>,
    ) -> Option<Ready> {
        let mut state = self.state.lock().expect("frame mailbox lock poisoned");
        loop {
            if state.closed {
                return None;
            }
            if let Some(ready) = ready(&mut state.pending) {
                return Some(ready);
            }
            state = self
                .available
                .wait(state)
                .expect("frame mailbox lock poisoned");
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[test]
    fn decoder_event_wakes_encoder_without_a_capture_frame() {
        let mailbox = Arc::new(FrameMailbox::<()>::new());
        let ready = Arc::new(AtomicBool::new(false));
        let (checking, encoder_waiting) = mpsc::channel();
        let (finished, output) = mpsc::channel();
        let worker = {
            let mailbox = mailbox.clone();
            let ready = ready.clone();
            std::thread::spawn(move || {
                let mut checking = Some(checking);
                let result = mailbox.wait_for_event(|| {
                    if let Some(checking) = checking.take() {
                        checking.send(()).unwrap();
                    }
                    ready.load(Ordering::Acquire).then_some(())
                });
                finished.send(result).unwrap();
            })
        };
        encoder_waiting.recv_timeout(Duration::from_secs(1)).unwrap();
        ready.store(true, Ordering::Release);
        mailbox.notify();
        let result = output.recv_timeout(Duration::from_secs(1));
        mailbox.close();
        worker.join().unwrap();
        assert_eq!(result.unwrap(), Some(()));
    }

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
    fn receiver_exits_when_closed_without_output_capacity() {
        let mailbox = Arc::new(FrameMailbox::new());
        mailbox.publish(1, "pending");
        let receiver = mailbox.clone();
        let (waiting_sender, waiting_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut reported = false;
            let result = receiver.receive_when(|| {
                if !reported {
                    waiting_sender.send(()).unwrap();
                    reported = true;
                }
                None::<()>
            });
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
