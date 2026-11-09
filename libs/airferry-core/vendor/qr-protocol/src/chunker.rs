//! Splitting a byte blob into RaptorQ source-block-aligned slices.
//!
//! The actual RFC 6330 source-block partitioning (computing KL/KS/Z) is done
//! inside `raptorq`'s `Encoder::with_defaults`. This module provides thin
//! helpers to (a) zero-pad the input to a whole number of symbols and (b)
//! answer "how many source blocks / symbols will the encoder produce?" without
//! building the encoder twice.

use raptorq_core::Config;

/// Zero-pad `data` up to a whole multiple of `config.symbol_size`.
///
/// RaptorQ tolerates short trailing symbols, but padding keeps every QR
/// payload exactly `symbol_size` bytes — which simplifies rendering and
/// scanning (uniform module density) at the cost of negligible overhead.
pub fn pad_to_symbols(data: &[u8], config: Config) -> Vec<u8> {
    let t = config.symbol_size() as usize;
    let rem = data.len() % t;
    if rem == 0 {
        data.to_vec()
    } else {
        let mut out = Vec::with_capacity(data.len() + (t - rem));
        out.extend_from_slice(data);
        out.resize(data.len() + (t - rem), 0);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_to_whole_symbol() {
        let cfg = Config::default(); // T=1024
        let padded = pad_to_symbols(&[0u8; 1500], cfg);
        assert_eq!(padded.len(), 2048);
    }

    #[test]
    fn no_pad_when_aligned() {
        let cfg = Config::default();
        let padded = pad_to_symbols(&[0u8; 2048], cfg);
        assert_eq!(padded.len(), 2048);
    }
}
