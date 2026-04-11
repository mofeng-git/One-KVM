//! Resample capture PCM to 48 kHz stereo for Opus (fixed 20 ms / 960×2 samples).

const OUT_RATE: f64 = 48000.0;
const OPUS_STEREO_SAMPLES: usize = 960 * 2;

enum PipelineState {
    /// Native 48 kHz interleaved stereo: only buffer and slice into 20 ms blocks (no float work).
    Stereo48kPassthrough,
    /// Other rates / mono: linear interpolation to 48 kHz stereo.
    Resample {
        in_rate: u32,
        in_channels: u32,
        next_out_frame: u64,
        buffer_start_frame: u64,
    },
}

/// Converts incoming interleaved PCM to 48 kHz stereo, then exposes fixed 960×2-sample chunks.
pub struct Opus48kPcmBuffer {
    state: PipelineState,
    pending: Vec<i16>,
}

impl Opus48kPcmBuffer {
    pub fn new(in_rate: u32, in_channels: u32) -> Self {
        let ch = in_channels.max(1);
        let rate = in_rate.max(1);
        let state = if rate == 48000 && ch == 2 {
            PipelineState::Stereo48kPassthrough
        } else {
            PipelineState::Resample {
                in_rate: rate,
                in_channels: ch,
                next_out_frame: 0,
                buffer_start_frame: 0,
            }
        };
        Self {
            state,
            pending: Vec::new(),
        }
    }

    /// True when input is already 48 kHz stereo (no interpolation loop).
    #[cfg(test)]
    pub fn is_passthrough(&self) -> bool {
        matches!(self.state, PipelineState::Stereo48kPassthrough)
    }

    /// Append one capture block (`sample_rate` must match the rate this buffer was built for).
    pub fn push_interleaved(&mut self, data: &[i16]) {
        self.pending.extend_from_slice(data);
    }

    /// Drain as many 960×2 stereo S16LE samples (20 ms @ 48 kHz) as possible.
    pub fn pop_opus_frames(&mut self, out: &mut Vec<i16>) {
        match &mut self.state {
            PipelineState::Stereo48kPassthrough => {
                while self.pending.len() >= OPUS_STEREO_SAMPLES {
                    out.extend_from_slice(&self.pending[..OPUS_STEREO_SAMPLES]);
                    self.pending.drain(..OPUS_STEREO_SAMPLES);
                }
            }
            PipelineState::Resample {
                in_rate,
                in_channels,
                next_out_frame,
                buffer_start_frame,
            } => {
                let ch = *in_channels as usize;
                if ch == 0 {
                    return;
                }

                loop {
                    let batch_start = *next_out_frame;
                    let mut block = Vec::with_capacity(OPUS_STEREO_SAMPLES);
                    let mut complete = true;

                    for i in 0u64..960 {
                        let k = batch_start + i;
                        let p_abs = (k as f64) * (*in_rate as f64) / OUT_RATE;
                        let f_abs = p_abs.floor() as u64;
                        let frac = p_abs - f_abs as f64;

                        let f_rel = f_abs.saturating_sub(*buffer_start_frame) as usize;
                        if f_rel + 1 >= self.pending.len() / ch {
                            complete = false;
                            break;
                        }

                        let base0 = f_rel * ch;
                        let base1 = (f_rel + 1) * ch;

                        let (l, r) = if *in_channels >= 2 {
                            let l0 = self.pending[base0] as f64;
                            let l1 = self.pending[base1] as f64;
                            let r0 = self.pending[base0 + 1] as f64;
                            let r1 = self.pending[base1 + 1] as f64;
                            (l0 + frac * (l1 - l0), r0 + frac * (r1 - r0))
                        } else {
                            let m0 = self.pending[base0] as f64;
                            let m1 = self.pending[base1] as f64;
                            let v = m0 + frac * (m1 - m0);
                            (v, v)
                        };

                        block.push(clamp_f64_to_i16(l));
                        block.push(clamp_f64_to_i16(r));
                    }

                    if !complete || block.len() != OPUS_STEREO_SAMPLES {
                        break;
                    }

                    out.extend_from_slice(&block);
                    *next_out_frame = batch_start + 960;
                    trim_resample_prefix(
                        &mut self.pending,
                        *in_rate,
                        *next_out_frame,
                        buffer_start_frame,
                        ch,
                    );
                }
            }
        }
    }
}

fn trim_resample_prefix(
    pending: &mut Vec<i16>,
    in_rate: u32,
    next_out_frame: u64,
    buffer_start_frame: &mut u64,
    ch: usize,
) {
    if pending.is_empty() {
        return;
    }

    let p_next = (next_out_frame as f64) * (in_rate as f64) / OUT_RATE;
    let need_abs = p_next.floor() as u64;
    let keep_from_abs = need_abs.saturating_sub(1);
    if keep_from_abs <= *buffer_start_frame {
        return;
    }

    let drop_frames = (keep_from_abs - *buffer_start_frame) as usize;
    let drop_samples = drop_frames.saturating_mul(ch).min(pending.len());
    if drop_samples > 0 {
        pending.drain(0..drop_samples);
        *buffer_start_frame += drop_frames as u64;
    }
}

#[inline]
fn clamp_f64_to_i16(v: f64) -> i16 {
    v.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_48k_identity_tone_length() {
        let mut buf = Opus48kPcmBuffer::new(48000, 2);
        assert!(buf.is_passthrough());
        let mut chunk = vec![0i16; 960 * 2];
        for i in 0..960 {
            let s = (i as f32 * 0.1).sin() * 3000.0;
            chunk[2 * i] = s as i16;
            chunk[2 * i + 1] = s as i16;
        }
        buf.push_interleaved(&chunk);
        let mut out = Vec::new();
        buf.pop_opus_frames(&mut out);
        assert_eq!(out.len(), 960 * 2);
    }

    #[test]
    fn upsample_44k_to_48k_chunk() {
        let mut buf = Opus48kPcmBuffer::new(44100, 2);
        assert!(!buf.is_passthrough());
        let mut chunk = vec![0i16; 882 * 2];
        for i in 0..882 {
            chunk[2 * i] = (i as i16).wrapping_mul(10);
            chunk[2 * i + 1] = (i as i16).wrapping_mul(-7);
        }
        buf.push_interleaved(&chunk);
        let mut out = Vec::new();
        buf.pop_opus_frames(&mut out);
        assert_eq!(out.len(), 960 * 2, "expected one 20ms Opus block");
    }

    #[test]
    fn mono_48k_not_passthrough() {
        let buf = Opus48kPcmBuffer::new(48000, 1);
        assert!(!buf.is_passthrough());
    }
}
