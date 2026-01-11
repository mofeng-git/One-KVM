//! Unicode support for exFAT filesystem
//!
//! exFAT uses UTF-16LE encoding for file names and requires Unicode-aware
//! case-insensitive comparison. This module provides:
//! - Unicode uppercase conversion for name hash calculation
//! - Upcase table generation
//! - Unicode-aware file name comparison

/// Convert a UTF-16 code unit to uppercase
///
/// This function handles:
/// - ASCII letters (a-z)
/// - Latin Extended characters (à-ÿ, etc.)
/// - Greek letters (α-ω)
/// - Cyrillic letters (а-я)
/// - And other commonly used Unicode letters
///
/// For full Unicode support, we use Rust's built-in char::to_uppercase(),
/// but for exFAT name hash we need a simpler mapping that matches the upcase table.
pub fn to_uppercase_simple(ch: u16) -> u16 {
    match ch {
        // ASCII lowercase (a-z)
        0x0061..=0x007A => ch - 32,

        // Latin-1 Supplement lowercase letters (à-ö, ø-ÿ)
        0x00E0..=0x00F6 | 0x00F8..=0x00FE => ch - 32,

        // Latin Extended-A (selected common mappings)
        0x0101 => 0x0100, // ā -> Ā
        0x0103 => 0x0102, // ă -> Ă
        0x0105 => 0x0104, // ą -> Ą
        0x0107 => 0x0106, // ć -> Ć
        0x0109 => 0x0108, // ĉ -> Ĉ
        0x010B => 0x010A, // ċ -> Ċ
        0x010D => 0x010C, // č -> Č
        0x010F => 0x010E, // ď -> Ď
        0x0111 => 0x0110, // đ -> Đ
        0x0113 => 0x0112, // ē -> Ē
        0x0115 => 0x0114, // ĕ -> Ĕ
        0x0117 => 0x0116, // ė -> Ė
        0x0119 => 0x0118, // ę -> Ę
        0x011B => 0x011A, // ě -> Ě
        0x011D => 0x011C, // ĝ -> Ĝ
        0x011F => 0x011E, // ğ -> Ğ
        0x0121 => 0x0120, // ġ -> Ġ
        0x0123 => 0x0122, // ģ -> Ģ
        0x0125 => 0x0124, // ĥ -> Ĥ
        0x0127 => 0x0126, // ħ -> Ħ
        0x0129 => 0x0128, // ĩ -> Ĩ
        0x012B => 0x012A, // ī -> Ī
        0x012D => 0x012C, // ĭ -> Ĭ
        0x012F => 0x012E, // į -> Į
        0x0131 => 0x0049, // ı -> I (Turkish dotless i)
        0x0133 => 0x0132, // ĳ -> Ĳ
        0x0135 => 0x0134, // ĵ -> Ĵ
        0x0137 => 0x0136, // ķ -> Ķ
        0x013A => 0x0139, // ĺ -> Ĺ
        0x013C => 0x013B, // ļ -> Ļ
        0x013E => 0x013D, // ľ -> Ľ
        0x0140 => 0x013F, // ŀ -> Ŀ
        0x0142 => 0x0141, // ł -> Ł
        0x0144 => 0x0143, // ń -> Ń
        0x0146 => 0x0145, // ņ -> Ņ
        0x0148 => 0x0147, // ň -> Ň
        0x014B => 0x014A, // ŋ -> Ŋ
        0x014D => 0x014C, // ō -> Ō
        0x014F => 0x014E, // ŏ -> Ŏ
        0x0151 => 0x0150, // ő -> Ő
        0x0153 => 0x0152, // œ -> Œ
        0x0155 => 0x0154, // ŕ -> Ŕ
        0x0157 => 0x0156, // ŗ -> Ŗ
        0x0159 => 0x0158, // ř -> Ř
        0x015B => 0x015A, // ś -> Ś
        0x015D => 0x015C, // ŝ -> Ŝ
        0x015F => 0x015E, // ş -> Ş
        0x0161 => 0x0160, // š -> Š
        0x0163 => 0x0162, // ţ -> Ţ
        0x0165 => 0x0164, // ť -> Ť
        0x0167 => 0x0166, // ŧ -> Ŧ
        0x0169 => 0x0168, // ũ -> Ũ
        0x016B => 0x016A, // ū -> Ū
        0x016D => 0x016C, // ŭ -> Ŭ
        0x016F => 0x016E, // ů -> Ů
        0x0171 => 0x0170, // ű -> Ű
        0x0173 => 0x0172, // ų -> Ų
        0x0175 => 0x0174, // ŵ -> Ŵ
        0x0177 => 0x0176, // ŷ -> Ŷ
        0x017A => 0x0179, // ź -> Ź
        0x017C => 0x017B, // ż -> Ż
        0x017E => 0x017D, // ž -> Ž
        0x017F => 0x0053, // ſ -> S (long s)

        // Greek lowercase (α-ω and variants)
        0x03B1..=0x03C1 => ch - 32, // α-ρ -> Α-Ρ
        0x03C3..=0x03C9 => ch - 32, // σ-ω -> Σ-Ω
        0x03C2 => 0x03A3,           // ς (final sigma) -> Σ

        // Cyrillic lowercase (а-я)
        0x0430..=0x044F => ch - 32, // а-я -> А-Я

        // Cyrillic Extended (ѐ-џ)
        0x0450..=0x045F => ch - 80, // ѐ-џ -> Ѐ-Џ

        // No conversion needed
        _ => ch,
    }
}

/// Generate the exFAT upcase table
///
/// The upcase table maps every UTF-16 code unit (0x0000-0xFFFF) to its
/// uppercase equivalent. This is used by the filesystem for case-insensitive
/// file name comparison.
///
/// Returns a 128KB table (65536 entries × 2 bytes each)
pub fn generate_upcase_table() -> Vec<u8> {
    let mut table = Vec::with_capacity(65536 * 2);

    for i in 0u32..65536 {
        let upper = to_uppercase_simple(i as u16);
        table.extend_from_slice(&upper.to_le_bytes());
    }

    table
}

/// Calculate exFAT name hash
///
/// The name hash is a 16-bit value stored in the Stream Extension entry,
/// used for fast file name lookup. It's calculated from the uppercase
/// version of each UTF-16 character.
pub fn calculate_name_hash(name: &str) -> u16 {
    let mut hash: u16 = 0;

    for ch in name.encode_utf16() {
        let upper = to_uppercase_simple(ch);
        let bytes = upper.to_le_bytes();
        hash = hash.rotate_right(1).wrapping_add(bytes[0] as u16);
        hash = hash.rotate_right(1).wrapping_add(bytes[1] as u16);
    }

    hash
}

/// Compare two file names in a case-insensitive manner
///
/// This uses Unicode-aware lowercase comparison (via Rust's str::to_lowercase)
/// which is appropriate for user-facing file name matching.
pub fn names_equal_ignore_case(name1: &str, name2: &str) -> bool {
    name1.to_lowercase() == name2.to_lowercase()
}

/// Encode a string as UTF-16LE bytes
pub fn encode_utf16le(s: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for ch in s.encode_utf16() {
        bytes.extend_from_slice(&ch.to_le_bytes());
    }
    bytes
}

/// Decode UTF-16LE bytes to a String
///
/// Handles surrogate pairs for characters outside the BMP (like emoji)
pub fn decode_utf16le(bytes: &[u8]) -> String {
    if bytes.len() % 2 != 0 {
        return String::new();
    }

    let code_units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|&c| c != 0) // Stop at null terminator
        .collect();

    String::from_utf16_lossy(&code_units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_uppercase() {
        assert_eq!(to_uppercase_simple(b'a' as u16), b'A' as u16);
        assert_eq!(to_uppercase_simple(b'z' as u16), b'Z' as u16);
        assert_eq!(to_uppercase_simple(b'A' as u16), b'A' as u16);
        assert_eq!(to_uppercase_simple(b'0' as u16), b'0' as u16);
    }

    #[test]
    fn test_latin_extended_uppercase() {
        // é -> É
        assert_eq!(to_uppercase_simple(0x00E9), 0x00C9);
        // ñ -> Ñ
        assert_eq!(to_uppercase_simple(0x00F1), 0x00D1);
        // ü -> Ü
        assert_eq!(to_uppercase_simple(0x00FC), 0x00DC);
    }

    #[test]
    fn test_greek_uppercase() {
        // α -> Α
        assert_eq!(to_uppercase_simple(0x03B1), 0x0391);
        // ω -> Ω
        assert_eq!(to_uppercase_simple(0x03C9), 0x03A9);
        // ς (final sigma) -> Σ
        assert_eq!(to_uppercase_simple(0x03C2), 0x03A3);
    }

    #[test]
    fn test_cyrillic_uppercase() {
        // а -> А
        assert_eq!(to_uppercase_simple(0x0430), 0x0410);
        // я -> Я
        assert_eq!(to_uppercase_simple(0x044F), 0x042F);
    }

    #[test]
    fn test_name_hash() {
        // Same hash for different cases
        let hash1 = calculate_name_hash("Test.txt");
        let hash2 = calculate_name_hash("TEST.TXT");
        let hash3 = calculate_name_hash("test.txt");
        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn test_name_hash_unicode() {
        // Unicode names should produce consistent hashes
        let hash1 = calculate_name_hash("Привет.txt"); // Russian
        let hash2 = calculate_name_hash("ПРИВЕТ.TXT");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_utf16_encoding() {
        // ASCII
        let encoded = encode_utf16le("Test");
        assert_eq!(encoded, vec![b'T', 0, b'e', 0, b's', 0, b't', 0]);

        // CJK character (中)
        let encoded = encode_utf16le("中");
        assert_eq!(encoded, vec![0x2D, 0x4E]); // U+4E2D in little-endian

        // Emoji (😀) - surrogate pair
        let encoded = encode_utf16le("😀");
        // U+1F600 = D83D DE00 (surrogate pair)
        assert_eq!(encoded, vec![0x3D, 0xD8, 0x00, 0xDE]);
    }

    #[test]
    fn test_utf16_decoding() {
        // ASCII
        let decoded = decode_utf16le(&[b'T', 0, b'e', 0, b's', 0, b't', 0]);
        assert_eq!(decoded, "Test");

        // CJK character
        let decoded = decode_utf16le(&[0x2D, 0x4E]);
        assert_eq!(decoded, "中");

        // With null terminator
        let decoded = decode_utf16le(&[b'H', 0, b'i', 0, 0, 0, b'X', 0]);
        assert_eq!(decoded, "Hi");

        // Emoji (surrogate pair)
        let decoded = decode_utf16le(&[0x3D, 0xD8, 0x00, 0xDE]);
        assert_eq!(decoded, "😀");
    }

    #[test]
    fn test_names_equal_ignore_case() {
        assert!(names_equal_ignore_case("Test.txt", "TEST.TXT"));
        assert!(names_equal_ignore_case("файл.txt", "ФАЙЛ.TXT")); // Russian
        assert!(!names_equal_ignore_case("Test1.txt", "Test2.txt"));
    }

    #[test]
    fn test_upcase_table_size() {
        let table = generate_upcase_table();
        assert_eq!(table.len(), 65536 * 2); // 128KB
    }
}
