//! GPU tone mapping feeding the RKMPP dma-buf encoder.

use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::time::Duration;

use hwcodec::rkmpp_dmabuf::{DmaEncoder, DmaEncoderConfig, DmaFormat};
use tracing::info;

use super::colorimetry::PqSignal;
use super::layout::{PackedLayout, HEIGHT, NV12_BYTES, WIDTH};
use super::stage::{Packet, Sink, Stage};
use super::vulkan::{ToneMapper, ToneParams};

/// Two slots leave one for the GPU while the encoder still holds the other;
/// the third absorbs a late encode without stalling the converter.
const OUTPUT_SLOTS: usize = 3;
const DEFAULT_DMA_HEAP: &str = "/dev/dma_heap/system";

pub struct HdrConfig {
    pub layout: PackedLayout,
    pub signal: PqSignal,
    pub hevc: bool,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub gop: u32,
}

struct MppSink(DmaEncoder);

impl Sink for MppSink {
    fn encode(&mut self, slot: usize, pts_ms: i64, keyframe: bool) -> Result<Vec<u8>, String> {
        // SAFETY: `Stage` encodes a slot only after the GPU job that wrote it
        // has completed and starts the next job into it only after this call
        // returns. The encoder never gets a capture buffer, so there is no
        // V4L2 lease to extend and no refreshed descriptor.
        unsafe { self.0.encode(slot, NV12_BYTES, None, pts_ms, keyframe) }
    }

    fn reconfigure(&mut self, kbps: u32, gop: u32) -> Result<(), String> {
        self.0.reconfigure(kbps, gop)
    }
}

pub struct HdrEncoder {
    stage: Stage<ToneMapper, MppSink>,
}

/// `ONE_KVM_HDR_DMA_HEAP`: heap node for the NV12 outputs; `none` forces Vulkan
/// export. The default works on hosts that expose /dev/dma_heap.
fn dma_heap(setting: Option<&str>) -> Option<PathBuf> {
    match setting {
        None => Some(PathBuf::from(DEFAULT_DMA_HEAP)),
        Some("" | "none" | "0") => None,
        Some(path) => Some(PathBuf::from(path)),
    }
}

/// `ONE_KVM_HDR_PIPELINE=0` encodes each frame right after its conversion.
fn overlap(setting: Option<&str>) -> bool {
    setting != Some("0")
}

impl HdrEncoder {
    /// `capture` are the exported V4L2 buffers, in queue order.
    pub fn new(config: &HdrConfig, capture: Vec<(OwnedFd, usize)>) -> Result<Self, String> {
        let heap = dma_heap(std::env::var("ONE_KVM_HDR_DMA_HEAP").ok().as_deref());
        let overlap = overlap(std::env::var("ONE_KVM_HDR_PIPELINE").ok().as_deref());
        let mapper = ToneMapper::new(
            config.layout,
            ToneParams {
                full_range: config.signal.full_range,
                ..ToneParams::default()
            },
            capture,
            OUTPUT_SLOTS,
            heap.as_deref(),
        )?;
        let device = mapper.device_name().to_owned();
        let encoder = DmaEncoder::new(
            DmaEncoderConfig {
                width: WIDTH,
                height: HEIGHT,
                stride: WIDTH,
                format: DmaFormat::Nv12,
                hevc: config.hevc,
                fps: config.fps,
                bitrate_kbps: config.bitrate_kbps,
                gop: config.gop,
            },
            mapper.output_buffers()?,
        )?;
        info!(
            "HDR tone mapping on {device}: {:?} stride {} -> NV12 BT.709, {} fps, {}",
            config.layout.format,
            config.layout.stride,
            config.fps,
            if overlap {
                "GPU/encoder overlapped"
            } else {
                "serial"
            },
        );
        Ok(Self {
            stage: Stage::new(mapper, MppSink(encoder), OUTPUT_SLOTS, config.fps, overlap),
        })
    }

    /// Convert capture buffer `index` and return the packet that became ready.
    /// The buffer may be requeued as soon as this returns.
    pub fn process(
        &mut self,
        index: usize,
        pts_ms: i64,
        frame_time: Duration,
        take_keyframe: impl FnMut() -> bool,
    ) -> Result<Option<Packet>, String> {
        self.stage
            .process(index, pts_ms, frame_time, take_keyframe)
    }

    pub fn discard_pending(&mut self) {
        self.stage.discard_pending();
    }

    pub fn reconfigure(&mut self, kbps: u32, gop: u32) -> Result<(), String> {
        self.stage.reconfigure(kbps, gop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dma_heap_setting() {
        assert_eq!(dma_heap(None), Some(PathBuf::from(DEFAULT_DMA_HEAP)));
        assert_eq!(dma_heap(Some("none")), None);
        assert_eq!(dma_heap(Some("")), None);
        assert_eq!(
            dma_heap(Some("/dev/dma_heap/system-uncached")),
            Some(PathBuf::from("/dev/dma_heap/system-uncached"))
        );
    }

    #[test]
    fn pipeline_overlaps_unless_disabled() {
        assert!(overlap(None));
        assert!(overlap(Some("1")));
        assert!(!overlap(Some("0")));
    }
}
