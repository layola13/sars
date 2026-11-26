use sa_flatten::def_expr::{eval, normalize_defs, strip_inline_comment, DefExprError};
use sa_flatten::{flatten_source, FlattenOptions, ImportResolver};
use sa_ir::DefDecl;
use std::collections::HashMap;

struct NullResolver;

impl ImportResolver for NullResolver {
    fn resolve(&self, _path: &str, _from: &str) -> Option<String> {
        None
    }
}

fn defs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn b5_eval_add_mul_precedence() {
    assert_eq!(eval("1+2", &HashMap::new()), Ok(3));
    assert_eq!(eval("1 + 2 * 3", &HashMap::new()), Ok(7));
    assert_eq!(eval("2 * 3 + 4 * 5", &HashMap::new()), Ok(26));
}

#[test]
fn b5_eval_parentheses_and_nested_defs() {
    let d = defs(&[("A", "7"), ("B", "(A + 1) * 2")]);
    assert_eq!(eval("(A + 1) * 2", &d), Ok(16));
    assert_eq!(eval("B", &d), Ok(16));
    assert_eq!(eval("B * B", &d), Ok(256));
}

#[test]
fn b5_eval_hex_and_negative_bit_patterns() {
    assert_eq!(eval("0x10", &HashMap::new()), Ok(16));
    assert_eq!(eval("0Xff", &HashMap::new()), Ok(255));
    assert_eq!(eval("0xffffffffffffffff", &HashMap::new()), Ok(-1));
    assert_eq!(eval("-0x8000000000000000", &HashMap::new()), Ok(i64::MIN));
}

#[test]
fn b5_eval_min_int_decimal_and_unary_minus() {
    assert_eq!(eval("-9223372036854775807 - 1", &HashMap::new()), Ok(i64::MIN));
    assert_eq!(eval("-(1 + 2)", &HashMap::new()), Ok(-3));
}

#[test]
fn b5_eval_inline_comment_and_underscore_identifiers() {
    assert_eq!(strip_inline_comment("10 // row cap"), "10 ");
    let d = defs(&[("MY_VAL", "123")]);
    assert_eq!(eval(strip_inline_comment("10 // row cap"), &HashMap::new()), Ok(10));
    assert_eq!(eval("MY_VAL * 2", &d), Ok(246));
    assert_eq!(eval("A.B * 2", &defs(&[("A.B", "21")])), Ok(42));
}

#[test]
fn b5_eval_rejects_unknown_and_malformed_expressions() {
    assert_eq!(eval("1 +", &HashMap::new()), Err(DefExprError::InvalidExpression));
    assert_eq!(eval("(1 + 2", &HashMap::new()), Err(DefExprError::InvalidExpression));
    assert!(matches!(
        eval("MISSING + 1", &HashMap::new()),
        Err(DefExprError::UnknownDef(name)) if name == "MISSING"
    ));
}

#[test]
fn b5_eval_cycle_hits_recursion_limit() {
    let d = defs(&[("A", "B"), ("B", "A")]);
    assert_eq!(eval("A", &d), Err(DefExprError::RecursionLimit));
}

#[test]
fn b5_normalize_defs_resolves_chain_and_preserves_non_numeric() {
    let mut list = vec![
        DefDecl {
            name: "SIZE".into(),
            value: "1+2*3".into(),
            loc: Default::default(),
        },
        DefDecl {
            name: "TOTAL".into(),
            value: "SIZE + 1 // derived".into(),
            loc: Default::default(),
        },
        DefDecl {
            name: "SRC".into(),
            value: "r7".into(),
            loc: Default::default(),
        },
    ];
    let values = normalize_defs(&mut list);
    assert_eq!(list[0].value, "7");
    assert_eq!(list[1].value, "8");
    assert_eq!(list[2].value, "r7");
    assert_eq!(values.get("SIZE").map(String::as_str), Some("7"));
    assert_eq!(values.get("TOTAL").map(String::as_str), Some("8"));
}

#[test]
fn b5_flatten_evaluates_def_expression_in_instruction() {
    let src = "#def A = 1+2\n@main() -> i32:\nL_ENTRY:\n    x = add r0, @A\n    return x\n";
    let program = flatten_source(src, "main.sa", &NullResolver, &FlattenOptions::default())
        .unwrap_or_else(|diags| panic!("{diags:?}"));
    let instr = &program.function("main").unwrap().blocks[0].instrs[0];
    assert_eq!(instr.raw, "x = add r0, 3");
    assert!(program.defs.is_empty());
}

#[test]
fn b5_flatten_evaluates_nested_def_chain() {
    let src = "#def A = 2 * 3\n#def B = (A + 1) * 2\n@main() -> i32:\nL_ENTRY:\n    x = add r0, @B\n    return x\n";
    let program = flatten_source(src, "main.sa", &NullResolver, &FlattenOptions::default())
        .unwrap_or_else(|diags| panic!("{diags:?}"));
    let instr = &program.function("main").unwrap().blocks[0].instrs[0];
    assert_eq!(instr.raw, "x = add r0, 14");
}
