//! C2: Operand::parse strips exactly one capability prefix (sci
//! stripTextOperandPrefix); stacked prefixes are not silently collapsed.
use sa_ir::Operand;

#[test]
fn c2_single_prefix_still_stripped() {
    assert_eq!(Operand::parse("&a"), Operand::Reg("a".into()));
    assert_eq!(Operand::parse("^a"), Operand::Reg("a".into()));
    assert_eq!(Operand::parse("*a"), Operand::Reg("a".into()));
    assert_eq!(Operand::parse(" &a "), Operand::Reg("a".into()));
}

#[test]
fn c2_stacked_prefixes_keep_the_remainder() {
    // Old sars looped and collapsed "&&a" -> Reg("a"); sci removes one prefix
    // only, leaving "&a" as the (invalid) name for a later stage to reject.
    assert_eq!(Operand::parse("&&a"), Operand::Reg("&a".into()));
    assert_eq!(Operand::parse("^^a"), Operand::Reg("^a".into()));
    assert_eq!(Operand::parse("&^a"), Operand::Reg("^a".into()));
    assert_eq!(Operand::parse("**a"), Operand::Reg("*a".into()));
    assert_ne!(Operand::parse("&&a"), Operand::parse("&a"));
}

#[test]
fn c2_at_prefix_is_symbol_not_cap() {
    assert_eq!(Operand::parse("@foo"), Operand::Symbol("foo".into()));
    assert_eq!(Operand::parse("&@foo"), Operand::Symbol("foo".into()));
}
