#[cfg(feature = "std")]
use std::ops::{Add, AddAssign, Div, Mul, Sub};

#[cfg(not(feature = "std"))]
use core::ops::{Add, AddAssign, Div, Mul, Sub};

#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};

// As defined in section 5.7.3
#[rustfmt::skip]
const OCT_EXP: [u8; 510] = [
   1, 2, 4, 8, 16, 32, 64, 128, 29, 58, 116, 232, 205, 135, 19, 38, 76,
   152, 45, 90, 180, 117, 234, 201, 143, 3, 6, 12, 24, 48, 96, 192, 157,
   39, 78, 156, 37, 74, 148, 53, 106, 212, 181, 119, 238, 193, 159, 35,
   70, 140, 5, 10, 20, 40, 80, 160, 93, 186, 105, 210, 185, 111, 222,
   161, 95, 190, 97, 194, 153, 47, 94, 188, 101, 202, 137, 15, 30, 60,
   120, 240, 253, 231, 211, 187, 107, 214, 177, 127, 254, 225, 223, 163,
   91, 182, 113, 226, 217, 175, 67, 134, 17, 34, 68, 136, 13, 26, 52,
   104, 208, 189, 103, 206, 129, 31, 62, 124, 248, 237, 199, 147, 59,
   118, 236, 197, 151, 51, 102, 204, 133, 23, 46, 92, 184, 109, 218,
   169, 79, 158, 33, 66, 132, 21, 42, 84, 168, 77, 154, 41, 82, 164, 85,
   170, 73, 146, 57, 114, 228, 213, 183, 115, 230, 209, 191, 99, 198,
   145, 63, 126, 252, 229, 215, 179, 123, 246, 241, 255, 227, 219, 171,
   75, 150, 49, 98, 196, 149, 55, 110, 220, 165, 87, 174, 65, 130, 25,
   50, 100, 200, 141, 7, 14, 28, 56, 112, 224, 221, 167, 83, 166, 81,
   162, 89, 178, 121, 242, 249, 239, 195, 155, 43, 86, 172, 69, 138, 9,
   18, 36, 72, 144, 61, 122, 244, 245, 247, 243, 251, 235, 203, 139, 11,
   22, 44, 88, 176, 125, 250, 233, 207, 131, 27, 54, 108, 216, 173, 71,
   142, 1, 2, 4, 8, 16, 32, 64, 128, 29, 58, 116, 232, 205, 135, 19, 38,
   76, 152, 45, 90, 180, 117, 234, 201, 143, 3, 6, 12, 24, 48, 96, 192,
   157, 39, 78, 156, 37, 74, 148, 53, 106, 212, 181, 119, 238, 193, 159,
   35, 70, 140, 5, 10, 20, 40, 80, 160, 93, 186, 105, 210, 185, 111,
   222, 161, 95, 190, 97, 194, 153, 47, 94, 188, 101, 202, 137, 15, 30,
   60, 120, 240, 253, 231, 211, 187, 107, 214, 177, 127, 254, 225, 223,
   163, 91, 182, 113, 226, 217, 175, 67, 134, 17, 34, 68, 136, 13, 26,
   52, 104, 208, 189, 103, 206, 129, 31, 62, 124, 248, 237, 199, 147,
   59, 118, 236, 197, 151, 51, 102, 204, 133, 23, 46, 92, 184, 109, 218,
   169, 79, 158, 33, 66, 132, 21, 42, 84, 168, 77, 154, 41, 82, 164, 85,
   170, 73, 146, 57, 114, 228, 213, 183, 115, 230, 209, 191, 99, 198,
   145, 63, 126, 252, 229, 215, 179, 123, 246, 241, 255, 227, 219, 171,
   75, 150, 49, 98, 196, 149, 55, 110, 220, 165, 87, 174, 65, 130, 25,
   50, 100, 200, 141, 7, 14, 28, 56, 112, 224, 221, 167, 83, 166, 81,
   162, 89, 178, 121, 242, 249, 239, 195, 155, 43, 86, 172, 69, 138, 9,
   18, 36, 72, 144, 61, 122, 244, 245, 247, 243, 251, 235, 203, 139, 11,
   22, 44, 88, 176, 125, 250, 233, 207, 131, 27, 54, 108, 216, 173, 71,
   142];

// As defined in section 5.7.4, but with a prepended zero to make this zero indexed
#[rustfmt::skip]
const OCT_LOG: [u8; 256] = [
   0, 0, 1, 25, 2, 50, 26, 198, 3, 223, 51, 238, 27, 104, 199, 75, 4, 100,
   224, 14, 52, 141, 239, 129, 28, 193, 105, 248, 200, 8, 76, 113, 5,
   138, 101, 47, 225, 36, 15, 33, 53, 147, 142, 218, 240, 18, 130, 69,
   29, 181, 194, 125, 106, 39, 249, 185, 201, 154, 9, 120, 77, 228, 114,
   166, 6, 191, 139, 98, 102, 221, 48, 253, 226, 152, 37, 179, 16, 145,
   34, 136, 54, 208, 148, 206, 143, 150, 219, 189, 241, 210, 19, 92,
   131, 56, 70, 64, 30, 66, 182, 163, 195, 72, 126, 110, 107, 58, 40,
   84, 250, 133, 186, 61, 202, 94, 155, 159, 10, 21, 121, 43, 78, 212,
   229, 172, 115, 243, 167, 87, 7, 112, 192, 247, 140, 128, 99, 13, 103,
   74, 222, 237, 49, 197, 254, 24, 227, 165, 153, 119, 38, 184, 180,
   124, 17, 68, 146, 217, 35, 32, 137, 46, 55, 63, 209, 91, 149, 188,
   207, 205, 144, 135, 151, 178, 220, 252, 190, 97, 242, 86, 211, 171,
   20, 42, 93, 158, 132, 60, 57, 83, 71, 109, 65, 162, 31, 45, 67, 216,
   183, 123, 164, 118, 196, 23, 73, 236, 127, 12, 111, 246, 108, 161,
   59, 82, 41, 157, 85, 170, 251, 96, 134, 177, 187, 204, 62, 90, 203,
   89, 95, 176, 156, 169, 160, 81, 11, 245, 22, 235, 122, 117, 44, 215,
   79, 174, 213, 233, 230, 231, 173, 232, 116, 214, 244, 234, 168, 80,
   88, 175];

pub static OCTET_MUL: [[u8; 256]; 256] = calculate_octet_mul_table();

// See "Screaming Fast Galois Field Arithmetic Using Intel SIMD Instructions" by Plank et al.
// Further adapted to AVX2
#[cfg(any(feature = "std", test))]
pub const OCTET_MUL_HI_BITS: [[u8; 32]; 256] = calculate_octet_mul_hi_table();
#[cfg(any(feature = "std", test))]
pub const OCTET_MUL_LOW_BITS: [[u8; 32]; 256] = calculate_octet_mul_low_table();

const fn const_mul(x: usize, y: usize) -> u8 {
    return OCT_EXP[OCT_LOG[x] as usize + OCT_LOG[y] as usize];
}

#[cfg(any(feature = "std", test))]
const fn calculate_octet_mul_hi_table() -> [[u8; 32]; 256] {
    let mut result = [[0; 32]; 256];
    let mut i = 1;
    while i < 256 {
        let mut j = 1;
        while j < 16 {
            result[i][j] = const_mul(i, j << 4);
            result[i][j + 16] = const_mul(i, j << 4);
            j += 1;
        }
        i += 1;
    }
    return result;
}

#[cfg(any(feature = "std", test))]
const fn calculate_octet_mul_low_table() -> [[u8; 32]; 256] {
    let mut result = [[0; 32]; 256];
    let mut i = 1;
    while i < 256 {
        let mut j = 1;
        while j < 16 {
            result[i][j] = const_mul(i, j);
            result[i][j + 16] = const_mul(i, j);
            j += 1;
        }
        i += 1;
    }
    return result;
}

const fn calculate_octet_mul_table() -> [[u8; 256]; 256] {
    let mut result = [[0; 256]; 256];
    let mut i = 1;
    while i < 256 {
        let mut j = 1;
        while j < 256 {
            result[i][j] = const_mul(i, j);
            j += 1;
        }
        i += 1;
    }
    return result;
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Octet {
    value: u8,
}

impl Octet {
    #[inline]
    pub fn new(value: u8) -> Octet {
        Octet { value }
    }

    #[inline]
    pub fn zero() -> Octet {
        Octet { value: 0 }
    }

    #[inline]
    pub fn one() -> Octet {
        Octet { value: 1 }
    }

    #[inline]
    pub fn alpha(i: usize) -> Octet {
        assert!(i < 256);
        Octet { value: OCT_EXP[i] }
    }

    #[inline]
    pub fn byte(&self) -> u8 {
        self.value
    }

    #[inline]
    pub fn fma(&mut self, other1: &Octet, other2: &Octet) {
        if other1.value != 0 && other2.value != 0 {
            unsafe {
                // This is safe because value is a u8, and OCT_LOG is 256 elements long
                let log_u = *OCT_LOG.get_unchecked(other1.value as usize) as usize;
                let log_v = *OCT_LOG.get_unchecked(other2.value as usize) as usize;
                // This is safe because the sum of two values in OCT_LOG cannot exceed 509
                self.value ^= *OCT_EXP.get_unchecked(log_u + log_v)
            }
        }
    }
}

impl Add for Octet {
    type Output = Octet;

    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn add(self, other: Octet) -> Octet {
        Octet {
            // As defined in section 5.7.2, addition on octets is implemented as bitxor
            value: self.value ^ other.value,
        }
    }
}

impl<'b> Add<&'b Octet> for &Octet {
    type Output = Octet;

    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn add(self, other: &'b Octet) -> Octet {
        Octet {
            // As defined in section 5.7.2, addition on octets is implemented as bitxor
            value: self.value ^ other.value,
        }
    }
}

impl AddAssign for Octet {
    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl, clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, other: Octet) {
        self.value ^= other.value;
    }
}

impl<'a> AddAssign<&'a Octet> for Octet {
    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl, clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, other: &'a Octet) {
        self.value ^= other.value;
    }
}

impl Sub for Octet {
    type Output = Octet;

    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn sub(self, rhs: Octet) -> Octet {
        Octet {
            // As defined in section 5.7.2, subtraction on octets is implemented as bitxor
            value: self.value ^ rhs.value,
        }
    }
}

impl Mul for Octet {
    type Output = Octet;

    #[inline]
    fn mul(self, other: Octet) -> Octet {
        &self * &other
    }
}

impl<'b> Mul<&'b Octet> for &Octet {
    type Output = Octet;

    #[inline]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn mul(self, other: &'b Octet) -> Octet {
        // As defined in section 5.7.2, multiplication is implemented via the tables above
        if self.value == 0 || other.value == 0 {
            Octet { value: 0 }
        } else {
            unsafe {
                // This is safe because value is a u8, and OCT_LOG is 256 elements long
                let log_u = *OCT_LOG.get_unchecked(self.value as usize) as usize;
                let log_v = *OCT_LOG.get_unchecked(other.value as usize) as usize;
                // This is safe because the sum of two values in OCT_LOG cannot exceed 509
                Octet {
                    value: *OCT_EXP.get_unchecked(log_u + log_v),
                }
            }
        }
    }
}

impl Div for Octet {
    type Output = Octet;

    fn div(self, rhs: Octet) -> Octet {
        &self / &rhs
    }
}

impl<'b> Div<&'b Octet> for &Octet {
    type Output = Octet;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: &'b Octet) -> Octet {
        assert_ne!(0, rhs.value);
        // As defined in section 5.7.2, division is implemented via the tables above
        if self.value == 0 {
            Octet { value: 0 }
        } else {
            let log_u = OCT_LOG[self.value as usize] as usize;
            let log_v = OCT_LOG[rhs.value as usize] as usize;
            Octet {
                value: OCT_EXP[255 + log_u - log_v],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use crate::octet::OCT_EXP;
    use crate::octet::OCT_LOG;
    use crate::octet::OCTET_MUL_HI_BITS;
    use crate::octet::OCTET_MUL_LOW_BITS;
    use crate::octet::Octet;

    #[test]
    fn multiplication_tables() {
        for i in 0..=255 {
            for j in 0..=255 {
                let expected = Octet::new(i) * Octet::new(j);
                let low = OCTET_MUL_LOW_BITS[i as usize][(j & 0x0F) as usize];
                let hi = OCTET_MUL_HI_BITS[i as usize][((j & 0xF0) >> 4) as usize];
                assert_eq!(low ^ hi, expected.byte());
            }
        }
    }

    #[test]
    fn addition() {
        let octet = Octet {
            value: rand::rng().random(),
        };
        // See section 5.7.2. u is its own additive inverse
        assert_eq!(Octet::zero(), &octet + &octet);
    }

    #[test]
    fn multiplication_identity() {
        let octet = Octet {
            value: rand::rng().random(),
        };
        assert_eq!(octet, &octet * &Octet::one());
    }

    #[test]
    fn multiplicative_inverse() {
        let octet = Octet {
            value: rand::rng().random_range(1..255),
        };
        let one = Octet::one();
        assert_eq!(one, &octet * &(&one / &octet));
    }

    #[test]
    fn division() {
        let octet = Octet {
            value: rand::rng().random_range(1..255),
        };
        assert_eq!(Octet::one(), &octet / &octet);
    }

    #[test]
    fn unsafe_mul_gaurantees() {
        let max_value = *OCT_LOG.iter().max().unwrap() as usize;
        assert!(2 * max_value < OCT_EXP.len());
    }

    #[test]
    fn fma() {
        let mut result = Octet::zero();
        let mut fma_result = Octet::zero();
        for i in 0..255 {
            for j in 0..255 {
                result += Octet::new(i) * Octet::new(j);
                fma_result.fma(&Octet::new(i), &Octet::new(j));
                assert_eq!(result, fma_result);
            }
        }
    }
}
