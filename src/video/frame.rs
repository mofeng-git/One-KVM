//! Video frame data structures

use bytes::Bytes;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use super::format::{PixelFormat, Resolution};

/// A video frame with metadata
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Raw frame data
    data: Arc<Bytes>,
    /// Cached xxHash64 of frame data (lazy computed for deduplication)
    hash: Arc<OnceLock<u64>>,
    /// Frame resolution
    pub resolution: Resolution,
    /// Pixel format
    pub format: PixelFormat,
    /// Stride (bytes per line)
    pub stride: u32,
    /// Whether this is a key frame (for compressed formats)
    pub key_frame: bool,
    /// Frame sequence number
    pub sequence: u64,
    /// Timestamp when frame was captured
    pub capture_ts: Instant,
    /// Whether capture is online (signal present)
    pub online: bool,
}

impl VideoFrame {
    /// Create a new video frame
    pub fn new(
        data: Bytes,
        resolution: Resolution,
        format: PixelFormat,
        stride: u32,
        sequence: u64,
    ) -> Self {
        Self {
            data: Arc::new(data),
            hash: Arc::new(OnceLock::new()),
            resolution,
            format,
            stride,
            key_frame: true,
            sequence,
            capture_ts: Instant::now(),
            online: true,
        }
    }

    /// Create a frame from a Vec<u8>
    pub fn from_vec(
        data: Vec<u8>,
        resolution: Resolution,
        format: PixelFormat,
        stride: u32,
        sequence: u64,
    ) -> Self {
        Self::new(Bytes::from(data), resolution, format, stride, sequence)
    }

    /// Get frame data as bytes slice
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get frame data as Bytes (cheap clone)
    pub fn data_bytes(&self) -> Bytes {
        (*self.data).clone()
    }

    /// Get data length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if frame is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get width
    pub fn width(&self) -> u32 {
        self.resolution.width
    }

    /// Get height
    pub fn height(&self) -> u32 {
        self.resolution.height
    }

    /// Get age of this frame (time since capture)
    pub fn age(&self) -> std::time::Duration {
        self.capture_ts.elapsed()
    }

    /// Check if this frame is still fresh (within threshold)
    pub fn is_fresh(&self, max_age_ms: u64) -> bool {
        self.age().as_millis() < max_age_ms as u128
    }

    /// Get hash of frame data (computed once, cached)
    /// Used for fast frame deduplication comparison
    pub fn get_hash(&self) -> u64 {
        *self.hash.get_or_init(|| {
            xxhash_rust::xxh64::xxh64(self.data.as_ref(), 0)
        })
    }

    /// Check if format is JPEG/MJPEG
    pub fn is_jpeg(&self) -> bool {
        self.format.is_compressed()
    }

    /// Validate JPEG frame data
    pub fn is_valid_jpeg(&self) -> bool {
        if !self.is_jpeg() {
            return false;
        }
        if self.data.len() < 125 {
            return false;
        }
        // Check JPEG header
        let start_marker = ((self.data[0] as u16) << 8) | self.data[1] as u16;
        if start_marker != 0xFFD8 {
            return false;
        }
        // Check JPEG end marker
        let end = self.data.len();
        let end_marker = ((self.data[end - 2] as u16) << 8) | self.data[end - 1] as u16;
        // Valid end markers: 0xFFD9, 0xD900, 0x0000 (padded)
        matches!(end_marker, 0xFFD9 | 0xD900 | 0x0000)
    }

    /// Create an offline placeholder frame
    pub fn offline(resolution: Resolution, format: PixelFormat) -> Self {
        Self {
            data: Arc::new(Bytes::new()),
            hash: Arc::new(OnceLock::new()),
            resolution,
            format,
            stride: 0,
            key_frame: true,
            sequence: 0,
            capture_ts: Instant::now(),
            online: false,
        }
    }
}

/// Frame metadata without actual data (for logging/stats)
#[derive(Debug, Clone)]
pub struct FrameMeta {
    pub resolution: Resolution,
    pub format: PixelFormat,
    pub size: usize,
    pub sequence: u64,
    pub key_frame: bool,
    pub online: bool,
}

impl From<&VideoFrame> for FrameMeta {
    fn from(frame: &VideoFrame) -> Self {
        Self {
            resolution: frame.resolution,
            format: frame.format,
            size: frame.len(),
            sequence: frame.sequence,
            key_frame: frame.key_frame,
            online: frame.online,
        }
    }
}

/// Ring buffer for storing recent frames
pub struct FrameRing {
    frames: Vec<Option<VideoFrame>>,
    capacity: usize,
    write_pos: usize,
    count: usize,
}

impl FrameRing {
    /// Create a new frame ring with specified capacity
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Ring capacity must be > 0");
        Self {
            frames: (0..capacity).map(|_| None).collect(),
            capacity,
            write_pos: 0,
            count: 0,
        }
    }

    /// Push a frame into the ring
    pub fn push(&mut self, frame: VideoFrame) {
        self.frames[self.write_pos] = Some(frame);
        self.write_pos = (self.write_pos + 1) % self.capacity;
        if self.count < self.capacity {
            self.count += 1;
        }
    }

    /// Get the latest frame
    pub fn latest(&self) -> Option<&VideoFrame> {
        if self.count == 0 {
            return None;
        }
        let pos = if self.write_pos == 0 {
            self.capacity - 1
        } else {
            self.write_pos - 1
        };
        self.frames[pos].as_ref()
    }

    /// Get number of frames in ring
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if ring is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Clear all frames
    pub fn clear(&mut self) {
        for frame in &mut self.frames {
            *frame = None;
        }
        self.write_pos = 0;
        self.count = 0;
    }
}
