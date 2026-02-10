//! Video frame data structures

use bytes::Bytes;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use super::format::{PixelFormat, Resolution};

#[derive(Clone)]
enum FrameData {
    Bytes(Bytes),
    Pooled(Arc<FrameBuffer>),
}

impl std::fmt::Debug for FrameData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameData::Bytes(bytes) => f
                .debug_struct("FrameData::Bytes")
                .field("len", &bytes.len())
                .finish(),
            FrameData::Pooled(buf) => f
                .debug_struct("FrameData::Pooled")
                .field("len", &buf.len())
                .finish(),
        }
    }
}

#[derive(Debug)]
pub struct FrameBufferPool {
    pool: Mutex<Vec<Vec<u8>>>,
    max_buffers: usize,
}

impl FrameBufferPool {
    pub fn new(max_buffers: usize) -> Self {
        Self {
            pool: Mutex::new(Vec::new()),
            max_buffers: max_buffers.max(1),
        }
    }

    pub fn take(&self, min_capacity: usize) -> Vec<u8> {
        let mut pool = self.pool.lock();
        if let Some(mut buf) = pool.pop() {
            if buf.capacity() < min_capacity {
                buf.reserve(min_capacity - buf.capacity());
            }
            buf
        } else {
            Vec::with_capacity(min_capacity)
        }
    }

    pub fn put(&self, mut buf: Vec<u8>) {
        buf.clear();
        let mut pool = self.pool.lock();
        if pool.len() < self.max_buffers {
            pool.push(buf);
        }
    }
}

pub struct FrameBuffer {
    data: Vec<u8>,
    pool: Option<Arc<FrameBufferPool>>,
}

impl FrameBuffer {
    pub fn new(data: Vec<u8>, pool: Option<Arc<FrameBufferPool>>) -> Self {
        Self { data, pool }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

impl std::fmt::Debug for FrameBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameBuffer")
            .field("len", &self.data.len())
            .finish()
    }
}

impl Drop for FrameBuffer {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.take() {
            let data = std::mem::take(&mut self.data);
            pool.put(data);
        }
    }
}

/// A video frame with metadata
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Raw frame data
    data: FrameData,
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
            data: FrameData::Bytes(data),
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

    /// Create a frame from pooled buffer
    pub fn from_pooled(
        data: Arc<FrameBuffer>,
        resolution: Resolution,
        format: PixelFormat,
        stride: u32,
        sequence: u64,
    ) -> Self {
        Self {
            data: FrameData::Pooled(data),
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

    /// Get frame data as bytes slice
    pub fn data(&self) -> &[u8] {
        match &self.data {
            FrameData::Bytes(bytes) => bytes,
            FrameData::Pooled(buf) => buf.as_slice(),
        }
    }

    /// Get frame data as Bytes (cheap clone)
    pub fn data_bytes(&self) -> Bytes {
        match &self.data {
            FrameData::Bytes(bytes) => bytes.clone(),
            FrameData::Pooled(buf) => Bytes::copy_from_slice(buf.as_slice()),
        }
    }

    /// Get data length
    pub fn len(&self) -> usize {
        self.data().len()
    }

    /// Check if frame is empty
    pub fn is_empty(&self) -> bool {
        self.data().is_empty()
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
        *self
            .hash
            .get_or_init(|| xxhash_rust::xxh64::xxh64(self.data(), 0))
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
        Self::is_valid_jpeg_bytes(self.data())
    }

    /// Validate JPEG bytes without constructing a frame
    pub fn is_valid_jpeg_bytes(data: &[u8]) -> bool {
        if data.len() < 125 {
            return false;
        }
        let start_marker = ((data[0] as u16) << 8) | data[1] as u16;
        if start_marker != 0xFFD8 {
            return false;
        }
        let end = data.len();
        let end_marker = ((data[end - 2] as u16) << 8) | data[end - 1] as u16;
        matches!(end_marker, 0xFFD9 | 0xD900 | 0x0000)
    }

    /// Create an offline placeholder frame
    pub fn offline(resolution: Resolution, format: PixelFormat) -> Self {
        Self {
            data: FrameData::Bytes(Bytes::new()),
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
