//! C1: strict UTF-8 literal validation at the const parse site.
use sa_parser::parse_source;

#[test]
fn c1_const_utf8_valid_literal_accepted() {
    let src = "@const A = utf8:\"x\\n\"\n@const B = \"A\\x42C\"\n";
    let prog = parse_source(src, "t.sa").expect("valid literals must parse");
    assert_eq!(prog.consts.len(), 2);
}

#[test]
fn c1_const_unknown_escape_rejected() {
    let src = "@const A = utf8:\"a\\q\"\n";
    let diags = parse_source(src, "t.sa").expect_err("unknown escape must be rejected");
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("invalid constant value")));
}

#[test]
fn c1_const_malformed_hex_rejected() {
    let src = "@const A = utf8:\"\\xZZ\"\n";
    assert!(parse_source(src, "t.sa").is_err());
}
