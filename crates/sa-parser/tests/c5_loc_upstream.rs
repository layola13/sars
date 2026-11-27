//! C5: `#loc` retargets the next instruction's SourceLoc to the upstream
//! source position (one-shot), matching sci flattener pending_loc.
use sa_parser::parse_source;

#[test]
fn c5_loc_retargets_next_instruction() {
    let src = concat!(
        "@main() -> i32:\nL_ENTRY:\n",
        "#loc \"upstream.sa\":42:7\n",
        "  nop\n",
        "  return 0\n",
        "",
    );
    let p = parse_source(src, "flat.sa").expect("parse");
    let instrs = &p.functions[0].blocks[0].instrs;
    assert_eq!(instrs.len(), 2);
    assert_eq!(instrs[0].loc.file, "upstream.sa");
    assert_eq!(instrs[0].loc.line, 42);
    assert_eq!(instrs[0].loc.column, 7);
    // Second instruction keeps the flat source location (one-shot consume).
    assert_eq!(instrs[1].loc.file, "flat.sa");
}

#[test]
fn c5_loc_decl_recorded_and_consumed_once() {
    let src = concat!(
        "@main() -> i32:\nL_ENTRY:\n",
        "#loc \"a.sa\":10:1\n",
        "  nop\n",
        "#loc \"b.sa\":20:2\n",
        "  nop\n",
        "  return 0\n",
        "",
    );
    let p = parse_source(src, "flat.sa").expect("parse");
    assert_eq!(p.locs.len(), 2);
    let instrs = &p.functions[0].blocks[0].instrs;
    assert_eq!(instrs[0].loc.file, "a.sa");
    assert_eq!(instrs[1].loc.file, "b.sa");
    assert_eq!(instrs[2].loc.file, "flat.sa");
}
