#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::octet::OCTET_MUL;
#[cfg(all(
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
    ),
    feature = "std"
))]
use crate::octet::OCTET_MUL_HI_BITS;
#[cfg(all(
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
    ),
    feature = "std"
))]
use crate::octet::OCTET_MUL_LOW_BITS;
use crate::octet::Octet;

#[cfg(all(target_arch = "aarch64", feature = "std"))]
use std::arch::is_aarch64_feature_detected;

// An octet vec containing only binary values, which are bit-packed for efficiency
pub struct BinaryOctetVec {
    // Values are stored packed into the highest bits, with the last value at the highest bit of the
    // last byte. Therefore, there may be trailing bits (least significant) which are unused
    elements: Vec<u64>,
    length: usize,
}

impl BinaryOctetVec {
    pub(crate) const WORD_WIDTH: usize = 64;

    pub fn new(elements: Vec<u64>, length: usize) -> Self {
        assert_eq!(elements.len(), length.div_ceil(Self::WORD_WIDTH));
        BinaryOctetVec { elements, length }
    }

    pub fn len(&self) -> usize {
        self.length
    }

    fn to_octet_vec(&self) -> Vec<u8> {
        let mut word = 0;
        let mut bit = self.padding_bits();

        let result = (0..self.length)
            .map(|_| {
                let value = u8::from(self.elements[word] & BinaryOctetVec::select_mask(bit) != 0);

                bit += 1;
                if bit == 64 {
                    word += 1;
                    bit = 0;
                }

                value
            })
            .collect();
        assert_eq!(word, self.elements.len());
        assert_eq!(bit, 0);
        result
    }

    pub fn padding_bits(&self) -> usize {
        (BinaryOctetVec::WORD_WIDTH - (self.length % BinaryOctetVec::WORD_WIDTH))
            % BinaryOctetVec::WORD_WIDTH
    }

    pub fn select_mask(bit: usize) -> u64 {
        1u64 << (bit as u64)
    }
}

#[inline]
pub fn fused_addassign_mul_scalar_binary(
    octets: &mut [u8],
    other: &BinaryOctetVec,
    scalar: &Octet,
) {
    debug_assert_ne!(
        *scalar,
        Octet::zero(),
        "Don't call with zero. It's very inefficient"
    );

    assert_eq!(octets.len(), other.len());
    if octets.is_empty() {
        return;
    }
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
    {
        if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("avx512bw") {
            unsafe {
                return fused_addassign_mul_scalar_binary_avx512(octets, other, scalar);
            }
        }
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("bmi1") {
            unsafe {
                return fused_addassign_mul_scalar_binary_avx2(octets, other, scalar);
            }
        }
    }
    #[cfg(all(target_arch = "aarch64", feature = "std"))]
    {
        if is_aarch64_feature_detected!("neon") {
            unsafe {
                return fused_addassign_mul_scalar_binary_neon(octets, other, scalar);
            }
        }
    }
    #[cfg(all(target_arch = "arm", feature = "std"))]
    {
        // TODO: enable when stable
        // if is_arm_feature_detected!("neon") {
        //     unsafe {
        //         return fused_addassign_mul_scalar_binary_neon(octets, other, scalar);
        //     }
        // }
    }

    // TODO: write an optimized fallback that does call .to_octet_vec()
    if *scalar == Octet::one() {
        return add_assign(octets, &other.to_octet_vec());
    } else {
        return fused_addassign_mul_scalar(octets, &other.to_octet_vec(), scalar);
    }
}

// TODO: enable when stable
#[cfg(all(target_arch = "aarch64", feature = "std"))]
// #[target_feature(enable = "neon")]
unsafe fn fused_addassign_mul_scalar_binary_neon(
    octets: &mut [u8],
    other: &BinaryOctetVec,
    scalar: &Octet,
) {
    unsafe {
        #[cfg(target_arch = "aarch64")]
        use std::arch::aarch64::*;
        #[cfg(target_arch = "arm")]
        use std::arch::arm::*;
        use std::mem;

        let first_bit = other.padding_bits();
        let other_u16 = std::slice::from_raw_parts(
            other.elements.as_ptr() as *const u16,
            other.elements.len() * 4,
        );
        let mut other_batch_start_index = first_bit / 16;
        let first_bits = other_u16[other_batch_start_index];
        let bit_in_first_bits = first_bit % 16;
        let mut remaining = octets.len();
        let mut self_neon_ptr = octets.as_mut_ptr();
        // Handle first bits to make remainder 16bit aligned
        if bit_in_first_bits > 0 {
            for (i, val) in octets.iter_mut().enumerate().take(16 - bit_in_first_bits) {
                // TODO: replace with UBFX instruction, once it's support in arm intrinsics
                let selected_bit = first_bits & (0x1 << (bit_in_first_bits + i));
                let other_byte = u8::from(selected_bit != 0);

                // other_byte is binary, so u8 multiplication is the same as GF256 multiplication
                *val ^= scalar.byte() * other_byte;
            }
            remaining -= 16 - bit_in_first_bits;
            other_batch_start_index += 1;
            self_neon_ptr = self_neon_ptr.add(16 - bit_in_first_bits);
        }

        assert_eq!(remaining % 16, 0);

        let shuffle_mask = vld1q_u8([0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1].as_ptr());
        let bit_select_mask = vld1q_u8(
            [
                1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80,
            ]
            .as_ptr(),
        );
        let scalar_neon = vdupq_n_u8(scalar.byte());
        let other_neon = other_u16.as_ptr();
        // Process the rest in 128bit chunks
        for i in 0..(remaining / mem::size_of::<uint8x16_t>()) {
            // Convert from bit packed u16 to 16xu8
            let other_vec = vld1q_dup_u16(other_neon.add(other_batch_start_index + i));
            let other_vec: uint8x16_t = mem::transmute(other_vec);
            let other_vec = vqtbl1q_u8(other_vec, shuffle_mask);
            let other_vec = vandq_u8(other_vec, bit_select_mask);
            // The bits are now unpacked, but aren't in a defined position (may be in 0-7 bit of each byte)

            // Test non-zero to get one or zero in the correct byte position
            let other_vec = vcgeq_u8(other_vec, vdupq_n_u8(1));
            // Multiply by scalar. other_vec is binary (0xFF or 0x00), so just mask with the scalar
            let product = vandq_u8(other_vec, scalar_neon);

            // Add to self
            let self_vec = vld1q_u8(self_neon_ptr.add(i * mem::size_of::<uint8x16_t>()));
            let result = veorq_u8(self_vec, product);
            store_neon((self_neon_ptr as *mut uint8x16_t).add(i), result);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "avx2")]
#[target_feature(enable = "bmi1")]
unsafe fn fused_addassign_mul_scalar_binary_avx2(
    octets: &mut [u8],
    other: &BinaryOctetVec,
    scalar: &Octet,
) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        let first_bit = other.padding_bits();
        let other_u32 = std::slice::from_raw_parts(
            other.elements.as_ptr() as *const u32,
            other.elements.len() * 2,
        );
        let mut other_batch_start_index = first_bit / 32;
        let first_bits = other_u32[other_batch_start_index];
        let bit_in_first_bits = first_bit % 32;
        let mut remaining = octets.len();
        let mut self_avx_ptr = octets.as_mut_ptr();
        // Handle first bits to make remainder 32bit aligned
        if bit_in_first_bits > 0 {
            let control = bit_in_first_bits as u32 | 0x100;
            for (i, val) in octets.iter_mut().enumerate().take(32 - bit_in_first_bits) {
                let other_byte = _bextr2_u32(first_bits, control + i as u32) as u8;
                // other_byte is binary, so u8 multiplication is the same as GF256 multiplication
                *val ^= scalar.byte() * other_byte;
            }
            remaining -= 32 - bit_in_first_bits;
            other_batch_start_index += 1;
            self_avx_ptr = self_avx_ptr.add(32 - bit_in_first_bits);
        }

        assert_eq!(remaining % 32, 0);

        // See: https://stackoverflow.com/questions/24225786/fastest-way-to-unpack-32-bits-to-a-32-byte-simd-vector
        let shuffle_mask = _mm256_set_epi64x(
            0x0303_0303_0303_0303,
            0x0202_0202_0202_0202,
            0x0101_0101_0101_0101,
            0,
        );
        let bit_select_mask = _mm256_set1_epi64x(0x8040_2010_0804_0201u64 as i64);
        let scalar_avx = _mm256_set1_epi8(scalar.byte() as i8);
        // Process the rest in 256bit chunks
        for i in 0..(remaining / 32) {
            // Convert from bit packed u32 to 32xu8
            let other_vec = _mm256_set1_epi32(other_u32[other_batch_start_index + i] as i32);
            let other_vec = _mm256_shuffle_epi8(other_vec, shuffle_mask);
            let other_vec = _mm256_andnot_si256(other_vec, bit_select_mask);
            // The bits are now unpacked, but aren't in a defined position (may be in 0-7 bit of each byte),
            // and are inverted (due to AND NOT)

            // Test against zero to get non-inverted selection
            let other_vec = _mm256_cmpeq_epi8(other_vec, _mm256_setzero_si256());
            // Multiply by scalar. other_vec is binary (0xFF or 0x00), so just mask with the scalar
            let product = _mm256_and_si256(other_vec, scalar_avx);

            // Add to self
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm256_loadu_si256((self_avx_ptr as *const __m256i).add(i));
            let result = _mm256_xor_si256(self_vec, product);
            #[allow(clippy::cast_ptr_alignment)]
            _mm256_storeu_si256((self_avx_ptr as *mut __m256i).add(i), result);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "avx512f")]
#[target_feature(enable = "avx512bw")]
unsafe fn fused_addassign_mul_scalar_binary_avx512(
    octets: &mut [u8],
    other: &BinaryOctetVec,
    scalar: &Octet,
) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        if octets.is_empty() {
            return;
        }
        let first_bit = other.padding_bits();
        let other_u64 = other.elements.as_ptr();
        let mut other_batch_start_index = first_bit / 64;
        let first_bits = *other_u64.add(other_batch_start_index);
        let bit_in_first_bits = first_bit % 64;
        let mut remaining = octets.len();
        let mut self_ptr = octets.as_mut_ptr();
        // Handle first bits to make remainder 64-bit aligned
        if bit_in_first_bits > 0 {
            for i in 0..(64 - bit_in_first_bits) {
                let other_byte = ((first_bits >> (bit_in_first_bits + i)) & 1) as u8;
                *self_ptr.add(i) ^= scalar.byte() * other_byte;
            }
            remaining -= 64 - bit_in_first_bits;
            other_batch_start_index += 1;
            self_ptr = self_ptr.add(64 - bit_in_first_bits);
        }

        assert_eq!(remaining % 64, 0);

        let scalar_avx = _mm512_set1_epi8(scalar.byte() as i8);
        // Process 64 bytes at a time: use masked move to broadcast scalar where bit=1
        for i in 0..(remaining / 64) {
            let bits = *other_u64.add(other_batch_start_index + i);
            // LDPC rows are sparse; most u64 words are zero
            if bits == 0 {
                continue;
            }
            // Single instruction: keep scalar byte where mask bit=1, zero where bit=0
            let product = _mm512_maskz_mov_epi8(bits, scalar_avx);

            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm512_loadu_si512(self_ptr.add(i * 64) as *const __m512i);
            let result = _mm512_xor_si512(self_vec, product);
            #[allow(clippy::cast_ptr_alignment)]
            _mm512_storeu_si512(self_ptr.add(i * 64) as *mut __m512i, result);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "avx512f")]
#[target_feature(enable = "avx512bw")]
unsafe fn mulassign_scalar_avx512(octets: &mut [u8], scalar: &Octet) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        let low_mask = _mm512_set1_epi8(0x0F);
        let self_ptr = octets.as_mut_ptr();
        let low_table128 =
            _mm_loadu_si128(OCTET_MUL_LOW_BITS[scalar.byte() as usize].as_ptr() as *const __m128i);
        let hi_table128 =
            _mm_loadu_si128(OCTET_MUL_HI_BITS[scalar.byte() as usize].as_ptr() as *const __m128i);
        let low_table = _mm512_broadcast_i32x4(low_table128);
        let hi_table = _mm512_broadcast_i32x4(hi_table128);

        for i in 0..(octets.len() / 64) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm512_loadu_si512(self_ptr.add(i * 64) as *const __m512i);
            let low = _mm512_and_si512(self_vec, low_mask);
            let low_result = _mm512_shuffle_epi8(low_table, low);
            let hi = _mm512_srli_epi64(self_vec, 4);
            let hi = _mm512_and_si512(hi, low_mask);
            let hi_result = _mm512_shuffle_epi8(hi_table, hi);
            let result = _mm512_xor_si512(hi_result, low_result);
            #[allow(clippy::cast_ptr_alignment)]
            _mm512_storeu_si512(self_ptr.add(i * 64) as *mut __m512i, result);
        }

        let remainder = octets.len() % 64;
        let scalar_index = usize::from(scalar.byte());
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) = *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(*octets.get_unchecked(i) as usize);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "avx512f")]
#[target_feature(enable = "avx512bw")]
unsafe fn fused_addassign_mul_scalar_avx512(octets: &mut [u8], other: &[u8], scalar: &Octet) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        let low_mask = _mm512_set1_epi8(0x0F);
        let self_ptr = octets.as_mut_ptr();
        let other_ptr = other.as_ptr();
        let low_table128 =
            _mm_loadu_si128(OCTET_MUL_LOW_BITS[scalar.byte() as usize].as_ptr() as *const __m128i);
        let hi_table128 =
            _mm_loadu_si128(OCTET_MUL_HI_BITS[scalar.byte() as usize].as_ptr() as *const __m128i);
        let low_table = _mm512_broadcast_i32x4(low_table128);
        let hi_table = _mm512_broadcast_i32x4(hi_table128);

        for i in 0..(octets.len() / 64) {
            #[allow(clippy::cast_ptr_alignment)]
            let other_vec = _mm512_loadu_si512(other_ptr.add(i * 64) as *const __m512i);
            let low = _mm512_and_si512(other_vec, low_mask);
            let low_result = _mm512_shuffle_epi8(low_table, low);
            let hi = _mm512_srli_epi64(other_vec, 4);
            let hi = _mm512_and_si512(hi, low_mask);
            let hi_result = _mm512_shuffle_epi8(hi_table, hi);
            let other_vec = _mm512_xor_si512(hi_result, low_result);

            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm512_loadu_si512(self_ptr.add(i * 64) as *const __m512i);
            let result = _mm512_xor_si512(self_vec, other_vec);
            #[allow(clippy::cast_ptr_alignment)]
            _mm512_storeu_si512(self_ptr.add(i * 64) as *mut __m512i, result);
        }

        let remainder = octets.len() % 64;
        let scalar_index = usize::from(scalar.byte());
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) ^= *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(*other.get_unchecked(i) as usize);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "avx512f")]
unsafe fn add_assign_avx512(octets: &mut [u8], other: &[u8]) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        assert_eq!(octets.len(), other.len());
        let self_ptr = octets.as_mut_ptr();
        let other_ptr = other.as_ptr();
        for i in 0..(octets.len() / 64) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm512_loadu_si512(self_ptr.add(i * 64) as *const __m512i);
            #[allow(clippy::cast_ptr_alignment)]
            let other_vec = _mm512_loadu_si512(other_ptr.add(i * 64) as *const __m512i);
            let result = _mm512_xor_si512(self_vec, other_vec);
            #[allow(clippy::cast_ptr_alignment)]
            _mm512_storeu_si512(self_ptr.add(i * 64) as *mut __m512i, result);
        }

        let remainder = octets.len() % 64;
        let self_ptr = octets.as_mut_ptr();
        let other_ptr = other.as_ptr();
        for i in ((octets.len() - remainder) / 8)..(octets.len() / 8) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_value = (self_ptr as *const u64).add(i).read_unaligned();
            #[allow(clippy::cast_ptr_alignment)]
            let other_value = (other_ptr as *const u64).add(i).read_unaligned();
            let result = self_value ^ other_value;
            #[allow(clippy::cast_ptr_alignment)]
            (self_ptr as *mut u64).add(i).write_unaligned(result);
        }

        let remainder = octets.len() % 8;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) ^= other.get_unchecked(i);
        }
    }
}

fn mulassign_scalar_fallback(octets: &mut [u8], scalar: &Octet) {
    let scalar_index = usize::from(scalar.byte());
    for item in octets {
        let octet_index = usize::from(*item);
        // SAFETY: `OCTET_MUL` is a 256x256 matrix, both indexes are `u8` inputs.
        *item = unsafe {
            *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(octet_index)
        };
    }
}

// TODO: enable when stable
#[cfg(all(target_arch = "aarch64", feature = "std"))]
// #[target_feature(enable = "neon")]
unsafe fn mulassign_scalar_neon(octets: &mut [u8], scalar: &Octet) {
    unsafe {
        #[cfg(target_arch = "aarch64")]
        use std::arch::aarch64::*;
        #[cfg(target_arch = "arm")]
        use std::arch::arm::*;
        use std::mem;

        let mask = vdupq_n_u8(0x0F);
        let self_neon_ptr = octets.as_mut_ptr();
        #[allow(clippy::cast_ptr_alignment)]
        let low_table = vld1q_u8(OCTET_MUL_LOW_BITS[scalar.byte() as usize].as_ptr());
        #[allow(clippy::cast_ptr_alignment)]
        let hi_table = vld1q_u8(OCTET_MUL_HI_BITS[scalar.byte() as usize].as_ptr());

        for i in 0..(octets.len() / mem::size_of::<uint8x16_t>()) {
            // Multiply by scalar
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = vld1q_u8(self_neon_ptr.add(i * mem::size_of::<uint8x16_t>()));
            let low = vandq_u8(self_vec, mask);
            let low_result = vqtbl1q_u8(low_table, low);
            let hi = vshrq_n_u8(self_vec, 4);
            let hi = vandq_u8(hi, mask);
            let hi_result = vqtbl1q_u8(hi_table, hi);
            let result = veorq_u8(hi_result, low_result);
            store_neon((self_neon_ptr as *mut uint8x16_t).add(i), result);
        }

        let remainder = octets.len() % mem::size_of::<uint8x16_t>();
        let scalar_index = scalar.byte() as usize;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) = *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(*octets.get_unchecked(i) as usize);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "avx2")]
unsafe fn mulassign_scalar_avx2(octets: &mut [u8], scalar: &Octet) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        let low_mask = _mm256_set1_epi8(0x0F);
        let hi_mask = _mm256_set1_epi8(0xF0u8 as i8);
        let self_avx_ptr = octets.as_mut_ptr();
        // Safe because _mm256_loadu_si256 loads from unaligned memory
        #[allow(clippy::cast_ptr_alignment)]
        let low_table = _mm256_loadu_si256(
            OCTET_MUL_LOW_BITS[scalar.byte() as usize].as_ptr() as *const __m256i
        );
        // Safe because _mm256_loadu_si256 loads from unaligned memory
        #[allow(clippy::cast_ptr_alignment)]
        let hi_table = _mm256_loadu_si256(
            OCTET_MUL_HI_BITS[scalar.byte() as usize].as_ptr() as *const __m256i
        );

        for i in 0..(octets.len() / 32) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm256_loadu_si256((self_avx_ptr as *const __m256i).add(i));
            let low = _mm256_and_si256(self_vec, low_mask);
            let low_result = _mm256_shuffle_epi8(low_table, low);
            let hi = _mm256_and_si256(self_vec, hi_mask);
            let hi = _mm256_srli_epi64(hi, 4);
            let hi_result = _mm256_shuffle_epi8(hi_table, hi);
            let result = _mm256_xor_si256(hi_result, low_result);
            #[allow(clippy::cast_ptr_alignment)]
            _mm256_storeu_si256((self_avx_ptr as *mut __m256i).add(i), result);
        }

        let remainder = octets.len() % 32;
        let scalar_index = scalar.byte() as usize;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) = *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(*octets.get_unchecked(i) as usize);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "ssse3")]
unsafe fn mulassign_scalar_ssse3(octets: &mut [u8], scalar: &Octet) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        let low_mask = _mm_set1_epi8(0x0F);
        let hi_mask = _mm_set1_epi8(0xF0u8 as i8);
        let self_ssse_ptr = octets.as_mut_ptr();
        #[allow(clippy::cast_ptr_alignment)]
        let low_table =
            _mm_loadu_si128(OCTET_MUL_LOW_BITS[scalar.byte() as usize].as_ptr() as *const __m128i);
        #[allow(clippy::cast_ptr_alignment)]
        let hi_table =
            _mm_loadu_si128(OCTET_MUL_HI_BITS[scalar.byte() as usize].as_ptr() as *const __m128i);

        for i in 0..(octets.len() / 16) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm_loadu_si128((self_ssse_ptr as *const __m128i).add(i));
            let low = _mm_and_si128(self_vec, low_mask);
            let low_result = _mm_shuffle_epi8(low_table, low);
            let hi = _mm_and_si128(self_vec, hi_mask);
            let hi = _mm_srli_epi64(hi, 4);
            let hi_result = _mm_shuffle_epi8(hi_table, hi);
            let result = _mm_xor_si128(hi_result, low_result);
            #[allow(clippy::cast_ptr_alignment)]
            _mm_storeu_si128((self_ssse_ptr as *mut __m128i).add(i), result);
        }

        let remainder = octets.len() % 16;
        let scalar_index = scalar.byte() as usize;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) = *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(*octets.get_unchecked(i) as usize);
        }
    }
}

#[inline]
pub fn mulassign_scalar(octets: &mut [u8], scalar: &Octet) {
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
    {
        if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("avx512bw") {
            unsafe {
                return mulassign_scalar_avx512(octets, scalar);
            }
        }
        if is_x86_feature_detected!("avx2") {
            unsafe {
                return mulassign_scalar_avx2(octets, scalar);
            }
        }
        if is_x86_feature_detected!("ssse3") {
            unsafe {
                return mulassign_scalar_ssse3(octets, scalar);
            }
        }
    }
    #[cfg(all(target_arch = "aarch64", feature = "std"))]
    {
        if is_aarch64_feature_detected!("neon") {
            unsafe {
                return mulassign_scalar_neon(octets, scalar);
            }
        }
    }
    #[cfg(all(target_arch = "arm", feature = "std"))]
    {
        // TODO: enable when stable
        // if is_arm_feature_detected!("neon") {
        //     unsafe {
        //         return mulassign_scalar_neon(octets, scalar);
        //     }
        // }
    }

    return mulassign_scalar_fallback(octets, scalar);
}

fn fused_addassign_mul_scalar_fallback(octets: &mut [u8], other: &[u8], scalar: &Octet) {
    let scalar_index = scalar.byte() as usize;
    for (i, octet) in octets.iter_mut().enumerate() {
        unsafe {
            *octet ^= *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(*other.get_unchecked(i) as usize);
        }
    }
}

// TODO: enable when stable
#[cfg(all(target_arch = "aarch64", feature = "std"))]
// #[target_feature(enable = "neon")]
unsafe fn fused_addassign_mul_scalar_neon(octets: &mut [u8], other: &[u8], scalar: &Octet) {
    unsafe {
        #[cfg(target_arch = "aarch64")]
        use std::arch::aarch64::*;
        #[cfg(target_arch = "arm")]
        use std::arch::arm::*;
        use std::mem;

        let mask = vdupq_n_u8(0x0F);
        let self_neon_ptr = octets.as_mut_ptr();
        let other_neon_ptr = other.as_ptr();
        #[allow(clippy::cast_ptr_alignment)]
        let low_table = vld1q_u8(OCTET_MUL_LOW_BITS[scalar.byte() as usize].as_ptr());
        #[allow(clippy::cast_ptr_alignment)]
        let hi_table = vld1q_u8(OCTET_MUL_HI_BITS[scalar.byte() as usize].as_ptr());

        for i in 0..(octets.len() / mem::size_of::<uint8x16_t>()) {
            // Multiply by scalar
            #[allow(clippy::cast_ptr_alignment)]
            let other_vec = vld1q_u8(other_neon_ptr.add(i * mem::size_of::<uint8x16_t>()));
            let low = vandq_u8(other_vec, mask);
            let low_result = vqtbl1q_u8(low_table, low);
            let hi = vshrq_n_u8(other_vec, 4);
            let hi = vandq_u8(hi, mask);
            let hi_result = vqtbl1q_u8(hi_table, hi);
            let other_vec = veorq_u8(hi_result, low_result);

            // Add to self
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = vld1q_u8(self_neon_ptr.add(i * mem::size_of::<uint8x16_t>()));
            let result = veorq_u8(self_vec, other_vec);
            #[allow(clippy::cast_ptr_alignment)]
            store_neon((self_neon_ptr as *mut uint8x16_t).add(i), result);
        }

        let remainder = octets.len() % mem::size_of::<uint8x16_t>();
        let scalar_index = scalar.byte() as usize;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) ^= *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(*other.get_unchecked(i) as usize);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "avx2")]
unsafe fn fused_addassign_mul_scalar_avx2(octets: &mut [u8], other: &[u8], scalar: &Octet) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        let low_mask = _mm256_set1_epi8(0x0F);
        let hi_mask = _mm256_set1_epi8(0xF0u8 as i8);
        let self_avx_ptr = octets.as_mut_ptr();
        let other_avx_ptr = other.as_ptr();
        // Safe because _mm256_loadu_si256 loads from unaligned memory
        #[allow(clippy::cast_ptr_alignment)]
        let low_table = _mm256_loadu_si256(
            OCTET_MUL_LOW_BITS[scalar.byte() as usize].as_ptr() as *const __m256i
        );
        // Safe because _mm256_loadu_si256 loads from unaligned memory
        #[allow(clippy::cast_ptr_alignment)]
        let hi_table = _mm256_loadu_si256(
            OCTET_MUL_HI_BITS[scalar.byte() as usize].as_ptr() as *const __m256i
        );

        for i in 0..(octets.len() / 32) {
            // Multiply by scalar
            #[allow(clippy::cast_ptr_alignment)]
            let other_vec = _mm256_loadu_si256((other_avx_ptr as *const __m256i).add(i));
            let low = _mm256_and_si256(other_vec, low_mask);
            let low_result = _mm256_shuffle_epi8(low_table, low);
            let hi = _mm256_and_si256(other_vec, hi_mask);
            let hi = _mm256_srli_epi64(hi, 4);
            let hi_result = _mm256_shuffle_epi8(hi_table, hi);
            let other_vec = _mm256_xor_si256(hi_result, low_result);

            // Add to self
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm256_loadu_si256((self_avx_ptr as *const __m256i).add(i));
            let result = _mm256_xor_si256(self_vec, other_vec);
            #[allow(clippy::cast_ptr_alignment)]
            _mm256_storeu_si256((self_avx_ptr as *mut __m256i).add(i), result);
        }

        let remainder = octets.len() % 32;
        let scalar_index = scalar.byte() as usize;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) ^= *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(*other.get_unchecked(i) as usize);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "ssse3")]
unsafe fn fused_addassign_mul_scalar_ssse3(octets: &mut [u8], other: &[u8], scalar: &Octet) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        let low_mask = _mm_set1_epi8(0x0F);
        let hi_mask = _mm_set1_epi8(0xF0u8 as i8);
        let self_ssse_ptr = octets.as_mut_ptr();
        let other_ssse_ptr = other.as_ptr();
        #[allow(clippy::cast_ptr_alignment)]
        let low_table =
            _mm_loadu_si128(OCTET_MUL_LOW_BITS[scalar.byte() as usize].as_ptr() as *const __m128i);
        #[allow(clippy::cast_ptr_alignment)]
        let hi_table =
            _mm_loadu_si128(OCTET_MUL_HI_BITS[scalar.byte() as usize].as_ptr() as *const __m128i);

        for i in 0..(octets.len() / 16) {
            // Multiply by scalar
            #[allow(clippy::cast_ptr_alignment)]
            let other_vec = _mm_loadu_si128((other_ssse_ptr as *const __m128i).add(i));
            let low = _mm_and_si128(other_vec, low_mask);
            let low_result = _mm_shuffle_epi8(low_table, low);
            let hi = _mm_and_si128(other_vec, hi_mask);
            let hi = _mm_srli_epi64(hi, 4);
            let hi_result = _mm_shuffle_epi8(hi_table, hi);
            let other_vec = _mm_xor_si128(hi_result, low_result);

            // Add to self
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm_loadu_si128((self_ssse_ptr as *const __m128i).add(i));
            let result = _mm_xor_si128(self_vec, other_vec);
            #[allow(clippy::cast_ptr_alignment)]
            _mm_storeu_si128((self_ssse_ptr as *mut __m128i).add(i), result);
        }

        let remainder = octets.len() % 16;
        let scalar_index = scalar.byte() as usize;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) ^= *OCTET_MUL
                .get_unchecked(scalar_index)
                .get_unchecked(*other.get_unchecked(i) as usize);
        }
    }
}

#[inline]
pub fn fused_addassign_mul_scalar(octets: &mut [u8], other: &[u8], scalar: &Octet) {
    debug_assert_ne!(
        *scalar,
        Octet::one(),
        "Don't call this with one. Use += instead"
    );
    debug_assert_ne!(
        *scalar,
        Octet::zero(),
        "Don't call with zero. It's very inefficient"
    );

    assert_eq!(octets.len(), other.len());
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
    {
        if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("avx512bw") {
            unsafe {
                return fused_addassign_mul_scalar_avx512(octets, other, scalar);
            }
        }
        if is_x86_feature_detected!("avx2") {
            unsafe {
                return fused_addassign_mul_scalar_avx2(octets, other, scalar);
            }
        }
        if is_x86_feature_detected!("ssse3") {
            unsafe {
                return fused_addassign_mul_scalar_ssse3(octets, other, scalar);
            }
        }
    }
    #[cfg(all(target_arch = "aarch64", feature = "std"))]
    {
        if is_aarch64_feature_detected!("neon") {
            unsafe {
                return fused_addassign_mul_scalar_neon(octets, other, scalar);
            }
        }
    }
    #[cfg(all(target_arch = "arm", feature = "std"))]
    {
        // TODO: enable when stable
        // if is_arm_feature_detected!("neon") {
        //     unsafe {
        //         return fused_addassign_mul_scalar_neon(octets, other, scalar);
        //     }
        // }
    }

    return fused_addassign_mul_scalar_fallback(octets, other, scalar);
}

fn add_assign_fallback(octets: &mut [u8], other: &[u8]) {
    assert_eq!(octets.len(), other.len());
    let self_ptr = octets.as_mut_ptr();
    let other_ptr = other.as_ptr();
    for i in 0..(octets.len() / 8) {
        unsafe {
            #[allow(clippy::cast_ptr_alignment)]
            let self_value = (self_ptr as *const u64).add(i).read_unaligned();
            #[allow(clippy::cast_ptr_alignment)]
            let other_value = (other_ptr as *const u64).add(i).read_unaligned();
            let result = self_value ^ other_value;
            #[allow(clippy::cast_ptr_alignment)]
            (self_ptr as *mut u64).add(i).write_unaligned(result);
        }
    }
    let remainder = octets.len() % 8;
    for i in (octets.len() - remainder)..octets.len() {
        unsafe {
            *octets.get_unchecked_mut(i) ^= other.get_unchecked(i);
        }
    }
}

#[cfg(all(target_arch = "aarch64", feature = "std"))]
use std::arch::aarch64::uint8x16_t;
// TODO: enable when stable
// #[cfg(target_arch = "arm")]
// use std::arch::arm::uint8x16_t;

// TODO: enable when stable
#[cfg(all(target_arch = "aarch64", feature = "std"))]
// #[target_feature(enable = "neon")]
unsafe fn store_neon(ptr: *mut uint8x16_t, value: uint8x16_t) {
    unsafe {
        #[cfg(target_arch = "aarch64")]
        use std::arch::aarch64::*;
        #[cfg(target_arch = "arm")]
        use std::arch::arm::*;

        // TODO: replace with vst1q_u8 when it's supported
        let reinterp = vreinterpretq_u64_u8(value);
        (ptr as *mut u64).write_unaligned(vgetq_lane_u64(reinterp, 0));
        (ptr as *mut u64)
            .add(1)
            .write_unaligned(vgetq_lane_u64(reinterp, 1));
    }
}

// TODO: enable when stable
#[cfg(all(target_arch = "aarch64", feature = "std"))]
// #[target_feature(enable = "neon")]
unsafe fn add_assign_neon(octets: &mut [u8], other: &[u8]) {
    unsafe {
        #[cfg(target_arch = "aarch64")]
        use std::arch::aarch64::*;
        #[cfg(target_arch = "arm")]
        use std::arch::arm::*;
        use std::mem;

        assert_eq!(octets.len(), other.len());
        let self_neon_ptr = octets.as_mut_ptr();
        let other_neon_ptr = other.as_ptr();
        for i in 0..(octets.len() / 16) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = vld1q_u8(self_neon_ptr.add(i * mem::size_of::<uint8x16_t>()));
            #[allow(clippy::cast_ptr_alignment)]
            let other_vec = vld1q_u8(other_neon_ptr.add(i * mem::size_of::<uint8x16_t>()));
            let result = veorq_u8(self_vec, other_vec);
            #[allow(clippy::cast_ptr_alignment)]
            store_neon((self_neon_ptr as *mut uint8x16_t).add(i), result);
        }

        let remainder = octets.len() % 16;
        let self_ptr = octets.as_mut_ptr();
        let other_ptr = other.as_ptr();
        for i in ((octets.len() - remainder) / 8)..(octets.len() / 8) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_value = (self_ptr as *mut u64).add(i).read_unaligned();
            #[allow(clippy::cast_ptr_alignment)]
            let other_value = (other_ptr as *mut u64).add(i).read_unaligned();
            let result = self_value ^ other_value;
            #[allow(clippy::cast_ptr_alignment)]
            (self_ptr as *mut u64).add(i).write_unaligned(result);
        }

        let remainder = octets.len() % 8;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) ^= other.get_unchecked(i);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "avx2")]
unsafe fn add_assign_avx2(octets: &mut [u8], other: &[u8]) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        assert_eq!(octets.len(), other.len());
        let self_avx_ptr = octets.as_mut_ptr();
        let other_avx_ptr = other.as_ptr();
        for i in 0..(octets.len() / 32) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm256_loadu_si256((self_avx_ptr as *const __m256i).add(i));
            #[allow(clippy::cast_ptr_alignment)]
            let other_vec = _mm256_loadu_si256((other_avx_ptr as *const __m256i).add(i));
            let result = _mm256_xor_si256(self_vec, other_vec);
            #[allow(clippy::cast_ptr_alignment)]
            _mm256_storeu_si256((self_avx_ptr as *mut __m256i).add(i), result);
        }

        let remainder = octets.len() % 32;
        let self_ptr = octets.as_mut_ptr();
        let other_ptr = other.as_ptr();
        for i in ((octets.len() - remainder) / 8)..(octets.len() / 8) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_value = (self_ptr as *mut u64).add(i).read_unaligned();
            #[allow(clippy::cast_ptr_alignment)]
            let other_value = (other_ptr as *mut u64).add(i).read_unaligned();
            let result = self_value ^ other_value;
            #[allow(clippy::cast_ptr_alignment)]
            (self_ptr as *mut u64).add(i).write_unaligned(result);
        }

        let remainder = octets.len() % 8;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) ^= other.get_unchecked(i);
        }
    }
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
#[target_feature(enable = "ssse3")]
unsafe fn add_assign_ssse3(octets: &mut [u8], other: &[u8]) {
    unsafe {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        assert_eq!(octets.len(), other.len());
        let self_ssse_ptr = octets.as_mut_ptr();
        let other_ssse_ptr = other.as_ptr();
        for i in 0..(octets.len() / 16) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_vec = _mm_loadu_si128((self_ssse_ptr as *const __m128i).add(i));
            #[allow(clippy::cast_ptr_alignment)]
            let other_vec = _mm_loadu_si128((other_ssse_ptr as *const __m128i).add(i));
            let result = _mm_xor_si128(self_vec, other_vec);
            #[allow(clippy::cast_ptr_alignment)]
            _mm_storeu_si128((self_ssse_ptr as *mut __m128i).add(i), result);
        }

        let remainder = octets.len() % 16;
        let self_ptr = octets.as_mut_ptr();
        let other_ptr = other.as_ptr();
        for i in ((octets.len() - remainder) / 8)..(octets.len() / 8) {
            #[allow(clippy::cast_ptr_alignment)]
            let self_value = (self_ptr as *mut u64).add(i).read_unaligned();
            #[allow(clippy::cast_ptr_alignment)]
            let other_value = (other_ptr as *mut u64).add(i).read_unaligned();
            let result = self_value ^ other_value;
            #[allow(clippy::cast_ptr_alignment)]
            (self_ptr as *mut u64).add(i).write_unaligned(result);
        }

        let remainder = octets.len() % 8;
        for i in (octets.len() - remainder)..octets.len() {
            *octets.get_unchecked_mut(i) ^= other.get_unchecked(i);
        }
    }
}

#[inline]
pub fn add_assign(octets: &mut [u8], other: &[u8]) {
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "std"))]
    {
        if is_x86_feature_detected!("avx512f") {
            unsafe {
                return add_assign_avx512(octets, other);
            }
        }
        if is_x86_feature_detected!("avx2") {
            unsafe {
                return add_assign_avx2(octets, other);
            }
        }
        if is_x86_feature_detected!("ssse3") {
            unsafe {
                return add_assign_ssse3(octets, other);
            }
        }
    }
    #[cfg(all(target_arch = "aarch64", feature = "std"))]
    {
        if is_aarch64_feature_detected!("neon") {
            unsafe {
                return add_assign_neon(octets, other);
            }
        }
    }
    #[cfg(all(target_arch = "arm", feature = "std"))]
    {
        // TODO: enable when stable
        // if is_arm_feature_detected!("neon") {
        //     unsafe {
        //         return add_assign_neon(octets, other);
        //     }
        // }
    }
    return add_assign_fallback(octets, other);
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use rand::Rng;
    use std::vec::Vec;

    use crate::octet::Octet;
    use crate::octets::mulassign_scalar;
    use crate::octets::{
        BinaryOctetVec, fused_addassign_mul_scalar, fused_addassign_mul_scalar_binary,
    };

    #[test]
    fn mul_assign() {
        let size: usize = 41;
        let scalar = Octet::new(rand::rng().random_range(1..255));
        let mut data1: Vec<u8> = vec![0; size];
        let mut expected: Vec<u8> = vec![0; size];
        for (d1, exp) in data1.iter_mut().zip(expected.iter_mut()) {
            *d1 = rand::rng().random();
            *exp = (&Octet::new(*d1) * &scalar).byte();
        }

        mulassign_scalar(&mut data1, &scalar);

        assert_eq!(expected, data1);
    }

    #[test]
    fn fma() {
        let size: usize = 41;
        let scalar = Octet::new(rand::rng().random_range(2..255));
        let mut data1: Vec<u8> = vec![0; size];
        let mut data2: Vec<u8> = vec![0; size];
        let mut expected: Vec<u8> = vec![0; size];
        for ((d1, d2), exp) in data1
            .iter_mut()
            .zip(data2.iter_mut())
            .zip(expected.iter_mut())
        {
            *d1 = rand::rng().random();
            *d2 = rand::rng().random();
            *exp = (Octet::new(*d1) + &Octet::new(*d2) * &scalar).byte();
        }

        fused_addassign_mul_scalar(&mut data1, &data2, &scalar);

        assert_eq!(expected, data1);
    }

    #[test]
    fn fma_binary() {
        let size: usize = 41;
        let scalar = Octet::new(rand::rng().random_range(2..255));
        let mut binary_vec: Vec<u64> = vec![0; size.div_ceil(64)];
        for item in binary_vec.iter_mut() {
            *item = rand::rng().random();
        }
        let binary_octet_vec = BinaryOctetVec::new(binary_vec, size);
        let mut data1: Vec<u8> = vec![0; size];
        let data2: Vec<u8> = binary_octet_vec.to_octet_vec();
        let mut expected: Vec<u8> = vec![0; size];
        for (i, (d1, exp)) in data1.iter_mut().zip(expected.iter_mut()).enumerate() {
            *d1 = rand::rng().random();
            *exp = (Octet::new(*d1) + &Octet::new(data2[i]) * &scalar).byte();
        }

        fused_addassign_mul_scalar_binary(&mut data1, &binary_octet_vec, &scalar);

        assert_eq!(expected, data1);
    }

    #[test]
    fn fma_binary_zero_len_noop() {
        let scalar = Octet::new(2);
        let binary_octet_vec = BinaryOctetVec::new(vec![], 0);
        let mut data: Vec<u8> = vec![];

        fused_addassign_mul_scalar_binary(&mut data, &binary_octet_vec, &scalar);

        assert!(data.is_empty());
    }
}
