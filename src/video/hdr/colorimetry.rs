//! Which captured signals the tone mapper may convert.
//!
//! The patched receiver reports the AVI/DRM InfoFrames through `G_FMT`:
//! BT.2020 colourspace plus the SMPTE ST 2084 transfer function means a PQ
//! signal. The shader implements exactly that mapping, so anything else is
//! refused rather than rendered with the wrong curve.

pub const V4L2_COLORSPACE_BT2020: u32 = 10;
pub const V4L2_XFER_FUNC_SMPTE2084: u32 = 7;
pub const V4L2_QUANTIZATION_FULL_RANGE: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PqSignal {
    pub full_range: bool,
}

pub fn classify(colorspace: u32, xfer_func: u32, quantization: u32) -> Option<PqSignal> {
    (colorspace == V4L2_COLORSPACE_BT2020 && xfer_func == V4L2_XFER_FUNC_SMPTE2084).then_some(
        PqSignal {
            full_range: quantization == V4L2_QUANTIZATION_FULL_RANGE,
        },
    )
}

/// Linear light (nits) for 4096 PQ code values, as read by the shader.
pub fn pq_table() -> Vec<f32> {
    const M1_INV: f64 = 1.0 / 0.159_301_757_812_5;
    const M2_INV: f64 = 1.0 / 78.843_75;
    const C1: f64 = 0.835_937_5;
    const C2: f64 = 18.851_562_5;
    const C3: f64 = 18.687_5;
    (0..4096)
        .map(|code| {
            let v = (code as f64 / 4095.0).powf(M2_INV);
            (10_000.0 * ((v - C1).max(0.0) / (C2 - C3 * v)).powf(M1_INV)) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bt2020_pq_is_converted() {
        assert_eq!(classify(10, 7, 2), Some(PqSignal { full_range: false }));
        assert_eq!(classify(10, 7, 1), Some(PqSignal { full_range: true }));
        assert_eq!(classify(10, 7, 0), Some(PqSignal { full_range: false }));
        // SDR BT.2020, HLG, sRGB and BT.709 PQ-less signals stay unsupported.
        assert_eq!(classify(10, 1, 2), None);
        assert_eq!(classify(10, 6, 2), None);
        assert_eq!(classify(8, 7, 2), None);
        assert_eq!(classify(3, 2, 2), None);
    }

    #[test]
    fn pq_table_spans_black_to_ten_thousand_nits() {
        let table = pq_table();
        assert_eq!(table.len(), 4096);
        assert_eq!(table[0], 0.0);
        assert!((table[4095] - 10_000.0).abs() < 1.0, "{}", table[4095]);
        assert!(table.windows(2).all(|pair| pair[0] <= pair[1]));
        // PQ code 0.5 is about 92 nits; 0.75 is about 1000 nits (BT.2100).
        assert!((table[2048] - 92.2).abs() < 1.0, "{}", table[2048]);
        assert!((table[3071] - 983.0).abs() < 25.0, "{}", table[3071]);
    }
}
