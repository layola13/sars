//! C4: `#version <digits>` is a recognised metadata line (sci LineKind.version).
use sa_parser::parse_source;

#[test]
fn c4_version_line_accepted() {
    let src = "#version 1\n@main() -> i32:\nL_ENTRY:\n  return 0\n";
    assert!(parse_source(src, "t.sa").is_ok(), "valid #version must parse");
}

#[test]
fn c4_version_multidigit_accepted() {
    let src = "#version 2026\n@main() -> i32:\nL_ENTRY:\n  return 0\n";
    assert!(parse_source(src, "t.sa").is_ok());
}

#[test]
fn c4_version_emits_no_instruction() {
    let src = "#version 1\n@main() -> i32:\nL_ENTRY:\n  return 0\n";
    let p = parse_source(src, "t.sa").unwrap();
    assert_eq!(p.functions[0].blocks[0].instrs.len(), 1);
}

#[test]
fn c4_invalid_version_rejected() {
    for bad in ["#version\n", "#version v1\n", "#version 1 2\n", "#version 1x\n"] {
        let src = format!("{bad}@main() -> i32:\nL_ENTRY:\n  return 0\n");
        assert!(parse_source(&src, "t.sa").is_err(), "invalid #version should fail: {bad:?}");
    }
}
