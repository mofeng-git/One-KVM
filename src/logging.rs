//! In-memory runtime log capture.
//!
//! A second `tracing` formatting layer mirrors every emitted event into a
//! bounded ring buffer and a broadcast channel. The Web UI uses this to show
//! the most recent log lines and to attach them to a problem report.

use std::collections::VecDeque;
use std::io;
use std::sync::{Mutex, OnceLock};

use tokio::sync::broadcast;

/// Number of formatted log lines kept for snapshots and reports.
pub const LOG_BUFFER_CAPACITY: usize = 1000;
/// Broadly matches the ring buffer; slow WebSocket clients get a `Lagged` notice.
const LOG_BROADCAST_CAPACITY: usize = 1024;
/// Individual log lines are truncated to keep the in-memory buffer bounded.
const MAX_LINE_BYTES: usize = 16 * 1024;
const TRUNCATION_SUFFIX: &str = " … (truncated)";

/// Bounded, concurrently readable runtime log buffer.
pub struct LogHub {
    lines: Mutex<VecDeque<String>>,
    sender: broadcast::Sender<String>,
}

impl LogHub {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(LOG_BROADCAST_CAPACITY);
        Self {
            lines: Mutex::new(VecDeque::with_capacity(LOG_BUFFER_CAPACITY)),
            sender,
        }
    }

    /// Append a log line, evicting the oldest entry when the buffer is full.
    ///
    /// Logging must never panic, so a poisoned lock is recovered from.
    pub fn push(&self, line: impl Into<String>) {
        let line = truncate_line(line.into());
        if line.is_empty() {
            return;
        }

        {
            let mut lines = self
                .lines
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if lines.len() >= LOG_BUFFER_CAPACITY {
                lines.pop_front();
            }
            lines.push_back(line.clone());
        }

        // An error only means nobody is currently subscribed.
        let _ = self.sender.send(line);
    }

    /// Return up to `limit` of the most recent lines, oldest first.
    pub fn snapshot(&self, limit: usize) -> Vec<String> {
        let lines = self
            .lines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let start = lines.len().saturating_sub(limit);
        lines.range(start..).cloned().collect()
    }

    /// Number of buffered lines.
    pub fn len(&self) -> usize {
        self.lines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Subscribe to lines emitted after this call.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }
}

impl Default for LogHub {
    fn default() -> Self {
        Self::new()
    }
}

static LOG_HUB: OnceLock<LogHub> = OnceLock::new();

/// Process-wide log hub shared by the tracing layer and the web handlers.
pub fn hub() -> &'static LogHub {
    LOG_HUB.get_or_init(LogHub::new)
}

fn truncate_line(mut line: String) -> String {
    let trimmed_len = line.trim_end().len();
    line.truncate(trimmed_len);
    if line.len() <= MAX_LINE_BYTES {
        return line;
    }

    let mut cut = MAX_LINE_BYTES - TRUNCATION_SUFFIX.len();
    while cut > 0 && !line.is_char_boundary(cut) {
        cut -= 1;
    }
    line.truncate(cut);
    line.push_str(TRUNCATION_SUFFIX);
    line
}

/// `MakeWriter` that feeds the shared [`LogHub`] instead of stdout.
#[derive(Clone, Copy, Default)]
pub struct LogCapture;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCapture {
    type Writer = LogCaptureWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogCaptureWriter::default()
    }
}

/// Accumulates one formatted event and publishes it when dropped.
///
/// `tracing-subscriber` may call `write` several times per event, so the
/// writer buffers until it is dropped.
#[derive(Default)]
pub struct LogCaptureWriter {
    buffer: Vec<u8>,
}

impl io::Write for LogCaptureWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for LogCaptureWriter {
    fn drop(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        let hub = hub();
        for line in lines_from_bytes(&self.buffer) {
            hub.push(line);
        }
    }
}

fn lines_from_bytes(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_keeps_only_the_newest_lines() {
        let hub = LogHub::new();
        for index in 0..(LOG_BUFFER_CAPACITY + 5) {
            hub.push(format!("line {index}"));
        }

        assert_eq!(hub.len(), LOG_BUFFER_CAPACITY);
        let snapshot = hub.snapshot(LOG_BUFFER_CAPACITY);
        assert_eq!(snapshot.len(), LOG_BUFFER_CAPACITY);
        assert_eq!(snapshot.first().unwrap(), "line 5");
        assert_eq!(
            snapshot.last().unwrap(),
            &format!("line {}", LOG_BUFFER_CAPACITY + 4)
        );
    }

    #[test]
    fn snapshot_respects_limit_and_order() {
        let hub = LogHub::new();
        for index in 0..10 {
            hub.push(format!("line {index}"));
        }

        assert_eq!(
            hub.snapshot(3),
            vec!["line 7".to_string(), "line 8".into(), "line 9".into()]
        );
    }

    #[test]
    fn subscribers_receive_new_lines() {
        let hub = LogHub::new();
        let mut receiver = hub.subscribe();

        hub.push("hello");

        assert_eq!(receiver.try_recv().unwrap(), "hello");
    }

    #[test]
    fn blank_lines_are_ignored_and_long_lines_truncated() {
        let hub = LogHub::new();
        hub.push("   ");
        assert!(hub.is_empty());

        let long = "x".repeat(MAX_LINE_BYTES + 100);
        hub.push(long);
        let snapshot = hub.snapshot(1);
        assert_eq!(snapshot.len(), 1);
        assert!(snapshot[0].len() <= MAX_LINE_BYTES);
        assert!(snapshot[0].ends_with(TRUNCATION_SUFFIX));
    }

    #[test]
    fn formatted_writer_splits_events_into_lines() {
        let lines = lines_from_bytes(b"one\r\ntwo\n\nthree");
        assert_eq!(lines, vec!["one", "two", "three"]);
    }
}
