//! C1: strict string escapes and UTF-8 validation, aligned with sci
//! `decodeQuotedBytes` / `utf8ValidateSlice`.
use sa_ir::{decode_quoted, unescape_bytes, utf8_valid, Operand};

#[test]
fn c1_known_escapes_decode() {
    let b = decode_quoted("\"a\\nb\\rc\\td\\0e\\\"f\\\\g\"").unwrap();
    assert_eq!(b, b"a\nb\rc\td\0e\"f\\g");
}

#[test]
fn c1_hex_escape_decodes_byte() {
    assert_eq!(decode_quoted("\"\\x00\\x41\\x7F\\xff\"").unwrap(), vec![0, 0x41, 0x7F, 0xFF]);
    assert_eq!(decode_quoted("\"A\\x42C\"").unwrap(), b"ABC");
}

#[test]
fn c1_unknown_escape_rejected() {
    assert!(decode_quoted("\"a\\q\"").is_err());
    assert!(decode_quoted("\"\\e\"").is_err());
    assert!(unescape_bytes(" trailing \\z ").is_err());
}

#[test]
fn c1_malformed_hex_rejected() {
    assert!(decode_quoted("\"\\x4\"").is_err());
    assert!(decode_quoted("\"\\xGG\"").is_err());
    assert!(decode_quoted("\"\\x\"").is_err());
}

#[test]
fn c1_trailing_backslash_rejected() {
    assert!(decode_quoted("\"abc\\").is_err());
    assert_eq!(decode_quoted("\"abc\\\\\""), Ok(b"abc\\".to_vec()));
}

#[test]
fn c1_quote_shape_validated() {
    assert!(decode_quoted("abc").is_err());
    assert!(decode_quoted("\"abc").is_err());
    assert!(decode_quoted("\"\"").unwrap().is_empty());
}

#[test]
fn c1_utf8_validation() {
    assert!(utf8_valid("héllo".as_bytes()));
    assert!(!utf8_valid(&[0xFF]));
    assert!(!utf8_valid(&[0xC3]));
    assert!(!utf8_valid(&[0xED, 0xA0, 0x80])); // surrogate
}

#[test]
fn c1_lenient_parse_path_zero_regression() {
    // Known escapes still decode through the live `Operand::parse` path.
    assert_eq!(Operand::parse("\"x\\n\""), Operand::Utf8("x\n".into()));
    assert_eq!(Operand::parse("utf8:\"x\\n\""), Operand::Utf8("x\n".into()));
    // Hex escape is newly recognised on the live path.
    assert_eq!(Operand::parse("\"\\x41\""), Operand::Utf8("A".into()));
    // Unknown escapes remain lenient on the live path (corpus compatibility).
    assert_eq!(Operand::parse("\"a\\q\""), Operand::Utf8("a\\q".into()));
}
