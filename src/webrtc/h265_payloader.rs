//! H.265/HEVC RTP Payloader
//!
//! Implements RFC 7798: RTP Payload Format for High Efficiency Video Coding (HEVC)
//!
//! H.265 NAL unit header (2 bytes):
//! ```text
//! +---------------+---------------+
//! |0|1|2|3|4|5|6|7|0|1|2|3|4|5|6|7|
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |F|   Type    |  LayerId  | TID |
//! +---------------+---------------+
//! ```
//!
//! Fragmentation Unit (FU) header:
//! ```text
//! +---------------+---------------+---------------+
//! |0|1|2|3|4|5|6|7|0|1|2|3|4|5|6|7|0|1|2|3|4|5|6|7|
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |F|   Type(49)  |  LayerId  | TID |S|E|  FuType |
//! +---------------+---------------+---------------+
//! ```
//!
//! Aggregation Packet (AP) for VPS+SPS+PPS:
//! ```text
//! +---------------+---------------+---------------+---------------+
//! |    PayloadHdr (Type=48)       |   NALU 1 Size |   NALU 1 Size |
//! +---------------+---------------+---------------+---------------+
//! |   NALU 1 HDR  |  NALU 1 Data  |   NALU 2 Size |   ...         |
//! +---------------+---------------+---------------+---------------+
//! ```

use bytes::{BufMut, Bytes, BytesMut};

/// H.265 NAL unit types (6 bits)
const H265_NAL_VPS: u8 = 32;
const H265_NAL_SPS: u8 = 33;
const H265_NAL_PPS: u8 = 34;
const H265_NAL_AUD: u8 = 35;
const H265_NAL_FILLER: u8 = 38;
#[allow(dead_code)]
const H265_NAL_SEI_PREFIX: u8 = 39;  // PREFIX_SEI_NUT
#[allow(dead_code)]
const H265_NAL_SEI_SUFFIX: u8 = 40;  // SUFFIX_SEI_NUT
#[allow(dead_code)]
const H265_NAL_AP: u8 = 48;  // Aggregation Packet
const H265_NAL_FU: u8 = 49;  // Fragmentation Unit

/// H.265 NAL header size
const H265_NAL_HEADER_SIZE: usize = 2;

/// FU header size (1 byte after NAL header)
const H265_FU_HEADER_SIZE: usize = 1;

/// Fixed PayloadHdr for FU packets: Type=49, LayerID=0, TID=1
/// This matches the rtp crate's FRAG_PAYLOAD_HDR
#[allow(dead_code)]
const FU_PAYLOAD_HDR: [u8; 2] = [0x62, 0x01];

/// Fixed PayloadHdr for AP packets: Type=48, LayerID=0, TID=1
/// This matches the rtp crate's AGGR_PAYLOAD_HDR
const AP_PAYLOAD_HDR: [u8; 2] = [0x60, 0x01];

/// H.265 RTP Payloader
///
/// Fragments H.265 NAL units for RTP transmission according to RFC 7798.
#[derive(Default, Debug, Clone)]
pub struct H265Payloader {
    /// Cached VPS NAL unit
    vps_nalu: Option<Bytes>,
    /// Cached SPS NAL unit
    sps_nalu: Option<Bytes>,
    /// Cached PPS NAL unit
    pps_nalu: Option<Bytes>,
}

impl H265Payloader {
    /// Create a new H265Payloader
    pub fn new() -> Self {
        Self::default()
    }

    /// Find the next Annex B start code in the NAL data
    fn next_ind(nalu: &Bytes, start: usize) -> (isize, isize) {
        let mut zero_count = 0;

        for (i, &b) in nalu[start..].iter().enumerate() {
            if b == 0 {
                zero_count += 1;
                continue;
            } else if b == 1 && zero_count >= 2 {
                return ((start + i - zero_count) as isize, zero_count as isize + 1);
            }
            zero_count = 0;
        }
        (-1, -1)
    }

    /// Extract NAL unit type from H.265 NAL header
    fn get_nal_type(nalu: &[u8]) -> u8 {
        if nalu.len() < 2 {
            return 0;
        }
        // Type is in bits 1-6 of the first byte
        (nalu[0] >> 1) & 0x3F
    }

    /// Emit a single NAL unit, fragmenting if necessary
    fn emit(&mut self, nalu: &Bytes, mtu: usize, payloads: &mut Vec<Bytes>) {
        if nalu.len() < H265_NAL_HEADER_SIZE {
            return;
        }

        let nal_type = Self::get_nal_type(nalu);

        // Skip AUD and filler data
        if nal_type == H265_NAL_AUD || nal_type == H265_NAL_FILLER {
            return;
        }

        // Cache parameter sets (VPS/SPS/PPS)
        match nal_type {
            H265_NAL_VPS => {
                self.vps_nalu = Some(nalu.clone());
                return; // Don't emit VPS separately, will be sent in AP
            }
            H265_NAL_SPS => {
                self.sps_nalu = Some(nalu.clone());
                return; // Don't emit SPS separately, will be sent in AP
            }
            H265_NAL_PPS => {
                self.pps_nalu = Some(nalu.clone());
                return; // Don't emit PPS separately, will be sent in AP
            }
            _ => {}
        }

        // Try to emit Aggregation Packet with VPS+SPS+PPS before video NAL
        self.try_emit_aggregation_packet(mtu, payloads);

        // Single NAL unit mode - if NAL fits in one packet
        if nalu.len() <= mtu {
            payloads.push(nalu.clone());
            return;
        }

        // Fragmentation Unit (FU) mode - fragment large NAL units
        self.emit_fragmented(nalu, mtu, payloads);
    }

    /// Try to emit an Aggregation Packet containing VPS+SPS+PPS
    fn try_emit_aggregation_packet(&mut self, mtu: usize, payloads: &mut Vec<Bytes>) {
        // Check if we have all three parameter sets
        let (vps, sps, pps) = match (&self.vps_nalu, &self.sps_nalu, &self.pps_nalu) {
            (Some(v), Some(s), Some(p)) => (v.clone(), s.clone(), p.clone()),
            _ => return,
        };

        // Calculate AP size: PayloadHdr(2) + 3x(NALU size(2) + NALU data)
        let ap_size = H265_NAL_HEADER_SIZE + 2 + vps.len() + 2 + sps.len() + 2 + pps.len();

        // Only create AP if it fits in MTU
        if ap_size > mtu {
            // Fall back to sending separately (as single NAL unit packets)
            payloads.push(vps);
            payloads.push(sps);
            payloads.push(pps);
            self.vps_nalu = None;
            self.sps_nalu = None;
            self.pps_nalu = None;
            return;
        }

        // Create Aggregation Packet
        let mut ap = BytesMut::with_capacity(ap_size);

        // PayloadHdr for AP (Type=48)
        ap.extend_from_slice(&AP_PAYLOAD_HDR);

        // VPS: size (2 bytes big-endian) + data
        ap.put_u16(vps.len() as u16);
        ap.extend_from_slice(&vps);

        // SPS: size (2 bytes big-endian) + data
        ap.put_u16(sps.len() as u16);
        ap.extend_from_slice(&sps);

        // PPS: size (2 bytes big-endian) + data
        ap.put_u16(pps.len() as u16);
        ap.extend_from_slice(&pps);

        payloads.push(ap.freeze());

        // Clear cached parameter sets
        self.vps_nalu = None;
        self.sps_nalu = None;
        self.pps_nalu = None;
    }

    /// Fragment a large NAL unit using FU packets
    fn emit_fragmented(&self, nalu: &Bytes, mtu: usize, payloads: &mut Vec<Bytes>) {
        if nalu.len() < H265_NAL_HEADER_SIZE {
            return;
        }

        // Get original NAL type for FU header
        let nal_type = Self::get_nal_type(nalu);

        // Maximum payload size per FU packet
        // FU packet = NAL header (2) + FU header (1) + payload
        let max_fragment_size = mtu - H265_NAL_HEADER_SIZE - H265_FU_HEADER_SIZE;

        if max_fragment_size == 0 {
            return;
        }

        // Skip the original NAL header, we'll create new FU headers
        let nalu_payload = &nalu[H265_NAL_HEADER_SIZE..];
        let full_nalu_size = nalu_payload.len();

        if full_nalu_size == 0 {
            return;
        }

        let mut offset = 0;

        while offset < full_nalu_size {
            let remaining = full_nalu_size - offset;
            let fragment_size = remaining.min(max_fragment_size);

            // Create FU packet
            let mut packet = BytesMut::with_capacity(H265_NAL_HEADER_SIZE + H265_FU_HEADER_SIZE + fragment_size);

            // NAL header for FU (2 bytes)
            // Preserve F bit (bit 7) and LayerID MSB (bit 0) from original, set Type to 49
            // This matches go2rtc approach: out[0] = (out[0] & 0b10000001) | (49 << 1)
            let byte0 = (nalu[0] & 0b10000001) | (H265_NAL_FU << 1);
            // Keep original byte1 (LayerID low 5 bits + TID) unchanged
            let byte1 = nalu[1];
            packet.put_u8(byte0);
            packet.put_u8(byte1);

            // FU header (1 byte)
            // S (1 bit) | E (1 bit) | FuType (6 bits)
            let mut fu_header = nal_type;
            if offset == 0 {
                fu_header |= 0x80; // S bit - start of fragmented NAL
            }
            if offset + fragment_size >= full_nalu_size {
                fu_header |= 0x40; // E bit - end of fragmented NAL
            }
            packet.put_u8(fu_header);

            // FU payload
            packet.put_slice(&nalu_payload[offset..offset + fragment_size]);

            payloads.push(packet.freeze());

            offset += fragment_size;
        }
    }
}

impl H265Payloader {
    /// Payload fragments H.265 packets across one or more RTP payloads
    ///
    /// Takes Annex B format NAL units (with start codes) and returns RTP payloads
    pub fn payload(&mut self, mtu: usize, payload: &Bytes) -> Vec<Bytes> {
        if payload.is_empty() || mtu == 0 {
            return vec![];
        }

        let mut payloads = vec![];

        // Parse Annex B format NAL units
        let (mut next_ind_start, mut next_ind_len) = Self::next_ind(payload, 0);
        if next_ind_start == -1 {
            // No start code found, treat entire payload as single NAL
            self.emit(payload, mtu, &mut payloads);
        } else {
            while next_ind_start != -1 {
                let prev_start = (next_ind_start + next_ind_len) as usize;
                let (next_ind_start2, next_ind_len2) = Self::next_ind(payload, prev_start);
                next_ind_start = next_ind_start2;
                next_ind_len = next_ind_len2;

                if next_ind_start != -1 {
                    self.emit(
                        &payload.slice(prev_start..next_ind_start as usize),
                        mtu,
                        &mut payloads,
                    );
                } else {
                    // Emit until end of stream
                    self.emit(&payload.slice(prev_start..), mtu, &mut payloads);
                }
            }
        }

        payloads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_nal_type() {
        // VPS (type 32): 0x40 = 0100 0000, type = 32
        assert_eq!(H265Payloader::get_nal_type(&[0x40, 0x01]), 32);
        // SPS (type 33): 0x42 = 0100 0010, type = 33
        assert_eq!(H265Payloader::get_nal_type(&[0x42, 0x01]), 33);
        // PPS (type 34): 0x44 = 0100 0100, type = 34
        assert_eq!(H265Payloader::get_nal_type(&[0x44, 0x01]), 34);
        // IDR (type 19): 0x26 = 0010 0110, type = 19
        assert_eq!(H265Payloader::get_nal_type(&[0x26, 0x01]), 19);
    }

    #[test]
    fn test_small_nalu() {
        let mut payloader = H265Payloader::new();
        // Small NAL that fits in MTU (no start code, just NAL data)
        let small_nal = Bytes::from(vec![0x26, 0x01, 0x00, 0x00, 0x00]); // IDR type
        let result = payloader.payload(1200, &small_nal);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], small_nal);
    }

    #[test]
    fn test_fragmentation() {
        let mut payloader = H265Payloader::new();
        // Large NAL that needs fragmentation
        let mut large_nal = vec![0x26, 0x01]; // IDR type header
        large_nal.extend(vec![0xAA; 2000]); // Payload
        let large_nal = Bytes::from(large_nal);

        let mtu = 1200;
        let result = payloader.payload(mtu, &large_nal);

        // Should be fragmented into multiple FU packets
        assert!(result.len() > 1);

        // Check first packet has S bit set
        assert_eq!(result[0][2] & 0x80, 0x80);

        // Check last packet has E bit set
        let last = result.last().unwrap();
        assert_eq!(last[2] & 0x40, 0x40);
    }

    #[test]
    fn test_fu_packet_format() {
        let mut payloader = H265Payloader::new();
        // IDR NAL: type=19, header = 0x26 0x01
        let mut nal = vec![0x26, 0x01]; // IDR type header (type=19, TID=1)
        nal.extend(vec![0xAA; 2000]); // Payload
        let nal = Bytes::from(nal);

        let mtu = 100; // Small MTU to force fragmentation
        let result = payloader.payload(mtu, &nal);

        // Verify FU packet structure
        for (i, pkt) in result.iter().enumerate() {
            assert!(pkt.len() >= 3, "Packet too short");

            // Check PayloadHdr (2 bytes)
            let byte0 = pkt[0];
            let byte1 = pkt[1];
            let nal_type = (byte0 >> 1) & 0x3F;

            assert_eq!(nal_type, 49, "PayloadHdr type should be 49 (FU)");
            // byte0 should be: (0x26 & 0x81) | (49 << 1) = 0x00 | 0x62 = 0x62
            assert_eq!(byte0, 0x62, "byte0 should be 0x62");
            // byte1 should be preserved from original: 0x01
            assert_eq!(byte1, 0x01, "byte1 should be 0x01");

            // Check FU header (1 byte)
            let fu_header = pkt[2];
            let fu_s = (fu_header >> 7) & 1;
            let fu_e = (fu_header >> 6) & 1;
            let fu_type = fu_header & 0x3F;

            assert_eq!(fu_type, 19, "FU type should be 19 (IDR)");

            if i == 0 {
                assert_eq!(fu_s, 1, "First packet should have S=1");
                assert_eq!(fu_e, 0, "First packet should have E=0");
            } else if i == result.len() - 1 {
                assert_eq!(fu_s, 0, "Last packet should have S=0");
                assert_eq!(fu_e, 1, "Last packet should have E=1");
            } else {
                assert_eq!(fu_s, 0, "Middle packet should have S=0");
                assert_eq!(fu_e, 0, "Middle packet should have E=0");
            }
        }
    }

    #[test]
    fn test_verify_with_rtp_depacketizer() {
        use rtp::codecs::h265::{H265Packet, H265Payload};
        use rtp::packetizer::Depacketizer;

        let mut payloader = H265Payloader::new();
        // Create IDR NAL with enough data to fragment
        let mut nal = vec![0x26, 0x01]; // IDR type=19
        nal.extend(vec![0xBB; 3000]);
        let nal = Bytes::from(nal);

        let result = payloader.payload(1200, &nal);
        assert!(result.len() > 1, "Should produce multiple FU packets");

        // Verify each packet can be depacketized by rtp crate
        for (i, pkt) in result.iter().enumerate() {
            let mut h265_pkt = H265Packet::default();
            let depack_result = h265_pkt.depacketize(pkt);

            assert!(
                depack_result.is_ok(),
                "Packet {} failed to depacketize: {:?}, bytes: {:02x?}",
                i,
                depack_result.err(),
                &pkt[..3.min(pkt.len())]
            );

            // Verify it's recognized as FU packet
            match h265_pkt.payload() {
                H265Payload::H265FragmentationUnitPacket(fu) => {
                    assert_eq!(fu.fu_header().fu_type(), 19, "FU type should be 19");
                    if i == 0 {
                        assert!(fu.fu_header().s(), "First packet S bit");
                    }
                    if i == result.len() - 1 {
                        assert!(fu.fu_header().e(), "Last packet E bit");
                    }
                }
                other => panic!("Expected FU packet, got {:?}", other),
            }
        }

        println!("All {} FU packets verified successfully!", result.len());
    }
}
