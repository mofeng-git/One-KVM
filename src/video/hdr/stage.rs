//! Two-deep GPU/encoder pipeline.
//!
//! Frame N is converted on the GPU while frame N-1 (already converted into a
//! different output slot) is being encoded, so throughput is bounded by the
//! slower of the two instead of their sum. The price is one frame of latency.
//! The capture buffer is released only after the GPU has finished reading it.

use std::time::Duration;

use super::pacing::Decimator;

pub trait Converter {
    /// Start converting capture buffer `input` into output slot `output`.
    fn submit(&mut self, input: usize, output: usize) -> Result<(), String>;
    /// Block until the submitted conversion has completed.
    fn wait(&mut self) -> Result<(), String>;
}

pub trait Sink {
    fn encode(&mut self, output: usize, pts_ms: i64, keyframe: bool) -> Result<Vec<u8>, String>;
    fn reconfigure(&mut self, kbps: u32, gop: u32) -> Result<(), String>;
}

#[derive(Debug)]
pub struct Packet {
    pub data: Vec<u8>,
    pub pts_ms: i64,
}

struct Pending {
    slot: usize,
    pts_ms: i64,
}

pub struct Stage<C: Converter, S: Sink> {
    // Field order is drop order: end encoder access to the output buffers
    // before the converter that owns them.
    sink: S,
    converter: C,
    outputs: usize,
    next: usize,
    pending: Option<Pending>,
    decimator: Decimator,
    overlap: bool,
}

impl<C: Converter, S: Sink> Stage<C, S> {
    pub fn new(converter: C, sink: S, outputs: usize, fps: u32, overlap: bool) -> Self {
        assert!(outputs >= 2 || !overlap, "overlap needs two output slots");
        Self {
            sink,
            converter,
            outputs: outputs.max(1),
            next: 0,
            pending: None,
            decimator: Decimator::new(fps),
            overlap,
        }
    }

    /// Frames dropped here never touch the GPU. Returns the packet for the
    /// previous frame when overlapping, or for this frame otherwise.
    pub fn process(
        &mut self,
        input: usize,
        pts_ms: i64,
        frame_time: Duration,
        mut take_keyframe: impl FnMut() -> bool,
    ) -> Result<Option<Packet>, String> {
        if !self.decimator.admit(frame_time) {
            return Ok(None);
        }
        let slot = self.next;
        self.converter.submit(input, slot)?;
        let encoded = if self.overlap {
            match self.pending.take() {
                Some(previous) => {
                    Some(self.encode(previous.slot, previous.pts_ms, &mut take_keyframe))
                }
                None => None,
            }
        } else {
            None
        };
        // The capture buffer must not be requeued while the GPU can still read
        // it, even if encoding the previous frame failed.
        let waited = self.converter.wait();
        let encoded = match (encoded, waited) {
            (_, Err(error)) => return Err(error),
            (Some(Err(error)), Ok(())) => return Err(error),
            (Some(Ok(packet)), Ok(())) => Some(packet),
            (None, Ok(())) => None,
        };
        self.next = (slot + 1) % self.outputs;
        if self.overlap {
            self.pending = Some(Pending { slot, pts_ms });
            Ok(encoded)
        } else {
            self.encode(slot, pts_ms, &mut take_keyframe).map(Some)
        }
    }

    fn encode(
        &mut self,
        slot: usize,
        pts_ms: i64,
        take_keyframe: &mut impl FnMut() -> bool,
    ) -> Result<Packet, String> {
        let keyframe = take_keyframe();
        self.sink
            .encode(slot, pts_ms, keyframe)
            .map(|data| Packet { data, pts_ms })
    }

    /// Forget the frame awaiting encode (nobody is watching, or the stream
    /// restarted), so a stale picture is never sent first.
    pub fn discard_pending(&mut self) {
        self.pending = None;
        self.decimator.reset();
    }

    pub fn reconfigure(&mut self, kbps: u32, gop: u32) -> Result<(), String> {
        self.sink.reconfigure(kbps, gop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    type Log = Rc<RefCell<Vec<String>>>;

    struct FakeGpu {
        log: Log,
        fail_submit: bool,
        fail_wait: bool,
    }
    impl Converter for FakeGpu {
        fn submit(&mut self, input: usize, output: usize) -> Result<(), String> {
            if self.fail_submit {
                return Err("submit".into());
            }
            self.log
                .borrow_mut()
                .push(format!("submit {input}->{output}"));
            Ok(())
        }
        fn wait(&mut self) -> Result<(), String> {
            self.log.borrow_mut().push("wait".into());
            if self.fail_wait {
                Err("wait".into())
            } else {
                Ok(())
            }
        }
    }

    struct FakeEncoder {
        log: Log,
        fail: bool,
    }
    impl Sink for FakeEncoder {
        fn encode(
            &mut self,
            output: usize,
            pts_ms: i64,
            keyframe: bool,
        ) -> Result<Vec<u8>, String> {
            self.log
                .borrow_mut()
                .push(format!("encode slot {output} pts {pts_ms} key {keyframe}"));
            if self.fail {
                Err("encode".into())
            } else {
                Ok(vec![output as u8])
            }
        }
        fn reconfigure(&mut self, _kbps: u32, _gop: u32) -> Result<(), String> {
            Ok(())
        }
    }

    fn stage(overlap: bool) -> (Stage<FakeGpu, FakeEncoder>, Log) {
        let log: Log = Rc::default();
        let stage = Stage::new(
            FakeGpu {
                log: log.clone(),
                fail_submit: false,
                fail_wait: false,
            },
            FakeEncoder {
                log: log.clone(),
                fail: false,
            },
            3,
            30,
            overlap,
        );
        (stage, log)
    }

    fn tick(n: u64) -> Duration {
        Duration::from_millis(34 * n)
    }

    #[test]
    fn overlapped_encode_runs_between_submit_and_wait() {
        let (mut stage, log) = stage(true);
        assert!(stage.process(2, 10, tick(0), || false).unwrap().is_none());
        let packet = stage.process(3, 20, tick(1), || true).unwrap().unwrap();
        assert_eq!((packet.data, packet.pts_ms), (vec![0], 10));
        assert_eq!(
            *log.borrow(),
            [
                "submit 2->0",
                "wait",
                "submit 3->1",
                "encode slot 0 pts 10 key true",
                "wait",
            ]
        );
    }

    #[test]
    fn output_slots_rotate_and_never_alias_the_pending_frame() {
        let (mut stage, log) = stage(true);
        for n in 0..6 {
            stage
                .process(n as usize % 4, n, tick(n as u64), || false)
                .unwrap();
        }
        let slots: Vec<usize> = log
            .borrow()
            .iter()
            .filter_map(|line| {
                line.strip_prefix("submit ")?
                    .split("->")
                    .nth(1)?
                    .parse()
                    .ok()
            })
            .collect();
        assert_eq!(slots, [0, 1, 2, 0, 1, 2]);
    }

    #[test]
    fn serial_mode_encodes_the_same_frame() {
        let (mut stage, _log) = stage(false);
        let packet = stage.process(0, 7, tick(0), || false).unwrap().unwrap();
        assert_eq!((packet.data, packet.pts_ms), (vec![0], 7));
    }

    #[test]
    fn sixty_hertz_input_is_halved_before_the_gpu() {
        let (mut stage, log) = stage(true);
        let base = tick(0);
        for n in 0..8u32 {
            let now = base + Duration::from_micros(16_667 * n as u64);
            stage.process(0, n as i64, now, || false).unwrap();
        }
        let submits = log
            .borrow()
            .iter()
            .filter(|l| l.starts_with("submit"))
            .count();
        assert_eq!(submits, 4);
    }

    #[test]
    fn capture_buffer_is_released_only_after_gpu_wait_even_when_encode_fails() {
        let log: Log = Rc::default();
        let mut stage = Stage::new(
            FakeGpu {
                log: log.clone(),
                fail_submit: false,
                fail_wait: false,
            },
            FakeEncoder {
                log: log.clone(),
                fail: true,
            },
            2,
            30,
            true,
        );
        stage.process(0, 1, tick(0), || false).unwrap();
        assert_eq!(
            stage.process(1, 2, tick(1), || false).unwrap_err(),
            "encode"
        );
        let log = log.borrow();
        let wait_after_submit = log
            .iter()
            .rposition(|l| l.starts_with("submit"))
            .map(|i| log[i..].iter().any(|l| l == "wait"));
        assert_eq!(wait_after_submit, Some(true));
    }

    #[test]
    fn gpu_errors_surface_and_nothing_is_left_pending() {
        let log: Log = Rc::default();
        let mut stage = Stage::new(
            FakeGpu {
                log: log.clone(),
                fail_submit: false,
                fail_wait: true,
            },
            FakeEncoder { log, fail: false },
            2,
            30,
            true,
        );
        assert_eq!(stage.process(0, 1, tick(0), || false).unwrap_err(), "wait");
        assert!(stage.pending.is_none());
    }

    #[test]
    fn discard_drops_the_stale_frame() {
        let (mut stage, log) = stage(true);
        stage.process(0, 1, tick(0), || false).unwrap();
        stage.discard_pending();
        assert!(stage.process(1, 2, tick(1), || false).unwrap().is_none());
        assert!(!log.borrow().iter().any(|l| l.starts_with("encode")));
    }
}
