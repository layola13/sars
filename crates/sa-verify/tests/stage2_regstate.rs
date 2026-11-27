use sa_ir::{
    Block, CapPrefix, Diagnostic, Function, Instruction, InstructionKind, Operand, ParamSpec,
    PrimType, Program, Signature, SourceLoc, Trap,
};
use sa_verify::regstate::CapabilityState;
use sa_verify::state::{FnState, RegKind, RegState, RegStateBackend};
use sa_verify::{verify, VerifyOptions};

fn loc(line: u32) -> SourceLoc {
    SourceLoc::new("test.sa", line, 1)
}

fn ins(kind: InstructionKind, line: u32) -> Instruction {
    Instruction::new(kind, loc(line), "")
}

fn block(label: &str, instrs: Vec<Instruction>, line: u32) -> Block {
    let mut result = Block::new(label, loc(line));
    result.instrs = instrs;
    result
}

fn function(name: &str, blocks: Vec<Block>) -> Function {
    let mut result = Function::new(Signature::new(name), loc(1));
    result.blocks = blocks;
    result
}

fn program(functions: Vec<Function>) -> Program {
    let mut result = Program::new();
    result.functions = functions;
    result
}

fn opts(regstate: bool) -> VerifyOptions {
    VerifyOptions { use_regstate: regstate, ..VerifyOptions::default() }
}

fn summary(result: &Result<sa_verify::VerifyReport, Vec<Diagnostic>>) -> Vec<(Trap, String)> {
    match result {
        Ok(_) => Vec::new(),
        Err(diags) => diags
            .iter()
            .map(|diag| (diag.trap, format!("{}|{:?}|{:?}", diag.message, diag.register, diag.hint)))
            .collect(),
    }
}

#[test]
fn stage3_new_path_is_default_on() {
    assert!(VerifyOptions::default().use_regstate);
    let off = opts(false);
    assert!(!off.use_regstate);
    assert_eq!(off.max_gas, VerifyOptions::default().max_gas);
    assert_eq!(off.check_leaks, VerifyOptions::default().check_leaks);
    assert_eq!(off.check_capabilities, VerifyOptions::default().check_capabilities);
    assert_eq!(off.check_phi, VerifyOptions::default().check_phi);
}

#[test]
fn stage2_one_entry_replaces_all_parallel_maps() {
    let mut legacy = FnState::default();
    legacy.set("a", RegState::Owned { kind: RegKind::Alloc });
    legacy.set_mask("a", 0x0401);
    legacy.fallible_insert("a", true);
    legacy.borrow_origin_insert("a", "base");
    legacy.origin_base_entry_or_insert("a", RegState::Owned { kind: RegKind::Param });
    legacy.escaped_insert("a");
    legacy.borrow_params_insert("a");

    let collapsed = CapabilityState::from(&legacy);
    assert_eq!(collapsed.entries().len(), 1);
    let entry = collapsed.entry("a").expect("single carrier");
    assert_eq!(entry.mask.map(|m| m.bits()), Some(0x0401));
    assert_eq!(collapsed.capability_mask("a").bits(), 0x0401);
    assert_eq!(collapsed.fallible_get("a"), Some(true));
    assert!(collapsed.borrow_origin_contains("a"));
    assert_eq!(
        collapsed.origin_base_get("a"),
        Some(RegState::Owned { kind: RegKind::Param })
    );
    assert!(collapsed.escaped_contains("a"));
    assert!(collapsed.borrow_params_contains("a"));

    let round: FnState = (&collapsed).into();
    assert!(FnState::is_same(&round, &legacy));
}

#[test]
fn stage2_state_operations_are_identical_on_both_backends() {
    let mut legacy = FnState::default();
    let mut collapsed = CapabilityState::default();
    for reg in ["p", "q", "v"] {
        legacy.set(reg, RegState::Owned { kind: RegKind::Param });
        collapsed.set(reg, RegState::Owned { kind: RegKind::Param });
    }
    legacy.borrow_params_insert("p");
    collapsed.borrow_params_insert("p");
    legacy.origin_base_entry_or_insert("q", RegState::Owned { kind: RegKind::Alloc });
    collapsed.origin_base_entry_or_insert("q", RegState::Owned { kind: RegKind::Alloc });
    legacy.set("q", RegState::Borrowed { shared: 2, mutable: false });
    collapsed.set("q", RegState::Borrowed { shared: 2, mutable: false });
    legacy.borrow_origin_insert("v", "q");
    collapsed.borrow_origin_insert("v", "q");
    legacy.set("v", RegState::Borrowed { shared: 1, mutable: true });
    collapsed.set("v", RegState::Borrowed { shared: 1, mutable: true });
    legacy.fallible_insert("v", false);
    collapsed.fallible_insert("v", false);
    legacy.escaped_insert("v");
    collapsed.escaped_insert("v");
    legacy.set_mask("q", 0x0011);
    collapsed.set_mask("q", 0x0011);
    legacy.set("p", RegState::Moved);
    collapsed.set("p", RegState::Moved);

    assert!(FnState::is_same(&(&collapsed).into(), &legacy));
    assert_eq!(collapsed.mask_of("q"), legacy.mask_of("q"));
    assert_eq!(collapsed.mask_of("v"), legacy.mask_of("v"));
    assert_eq!(collapsed.borrow_origin_remove("v"), legacy.borrow_origin_remove("v"));
    assert!(FnState::is_same(&(&collapsed).into(), &legacy));
    assert_eq!(collapsed.origin_base_remove("q"), legacy.origin_base_remove("q"));
    assert!(FnState::is_same(&(&collapsed).into(), &legacy));
    let mut collapsed_regs = collapsed.regs_snapshot();
    let mut legacy_regs = legacy.regs_snapshot();
    collapsed_regs.sort_by(|a, b| a.0.cmp(&b.0));
    legacy_regs.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(collapsed_regs, legacy_regs);
}

#[test]
fn stage2_merge_is_identical_on_both_backends() {
    let mut left = FnState::default();
    let mut right = FnState::default();
    left.set("a", RegState::Owned { kind: RegKind::Alloc });
    right.set("a", RegState::Owned { kind: RegKind::Alloc });
    left.set_mask("a", 0x0101);
    right.set_mask("a", 0x0001);
    left.set("b", RegState::Borrowed { shared: 1, mutable: false });
    right.set("b", RegState::Borrowed { shared: 1, mutable: false });
    left.fallible_insert("c", true);
    right.fallible_insert("c", true);
    left.fallible_insert("d", true);
    right.fallible_insert("d", false);
    left.borrow_origin_insert("e", "a");
    right.borrow_origin_insert("e", "a");
    left.escaped_insert("f");
    right.escaped_insert("f");
    left.borrow_params_insert("g");
    right.borrow_params_insert("g");

    let collapsed_left = CapabilityState::from(&left);
    let collapsed_right = CapabilityState::from(&right);
    let (legacy_merged, legacy_conflicts) = left.merge(&right);
    let (collapsed_merged, collapsed_conflicts) = collapsed_left.merge(&collapsed_right);
    assert_eq!(legacy_conflicts, collapsed_conflicts);
    let round: FnState = (&collapsed_merged).into();
    assert!(FnState::is_same(&round, &legacy_merged));
}

fn programs() -> Vec<Program> {
    let mut fallible_sig = Signature::new("fallible");
    fallible_sig.return_fallible = true;
    fallible_sig.return_ty = PrimType::Void;
    let mut fallible_call = ins(
        InstructionKind::Call { target: "fallible".into(), args: Vec::new() },
        2,
    );
    fallible_call.dest = Some("r".into());
    let mut try_ins = ins(InstructionKind::Try { src: Operand::Reg("r".into()) }, 3);
    try_ins.dest = Some("u".into());
    vec![
        program(vec![function(
            "valid",
            vec![block(
                "L_entry",
                vec![
                    ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                    ins(InstructionKind::Release { reg: "a".into() }, 3),
                    ins(InstructionKind::Return { value: None }, 4),
                ],
                1,
            )],
        )]),
        program(vec![function(
            "move",
            vec![block(
                "L_entry",
                vec![
                    ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                    ins(InstructionKind::Move { src: Operand::Reg("a".into()) }, 3)
                        .with_dest("b"),
                    ins(InstructionKind::Store {
                        addr: sa_ir::Address { base: Operand::Reg("b".into()), offset: 0 },
                        value: Operand::Int(1),
                    }, 4),
                    ins(InstructionKind::Load {
                        addr: sa_ir::Address { base: Operand::Reg("a".into()), offset: 0 },
                    }, 5)
                    .with_dest("c"),
                    ins(InstructionKind::Return { value: None }, 6),
                ],
                1,
            )],
        )]),
        {
            let mut main = Function::new(Signature::new("main"), loc(1));
            main.sig.params.push(ParamSpec {
                name: "arg".into(),
                ty: PrimType::Ptr,
                cap: CapPrefix::Borrow,
            });
            main.blocks.push(block(
                "L_entry",
                vec![ins(InstructionKind::Return { value: None }, 2)],
                1,
            ));
            program(vec![main])
        },
        {
            let mut callee = Function::new(fallible_sig, loc(1));
            callee.blocks.push(block(
                "L_entry",
                vec![ins(InstructionKind::Return { value: None }, 2)],
                1,
            ));
            let mut main = Function::new(Signature::new("main"), loc(1));
            main.blocks.push(block(
                "L_entry",
                vec![
                    fallible_call,
                    try_ins,
                    ins(InstructionKind::Return { value: None }, 4),
                ],
                1,
            ));
            program(vec![callee, main])
        },
    ]
}

#[test]
fn stage2_verification_is_identical_with_new_path_off_and_on() {
    for candidate in programs() {
        let off = verify(&candidate, &opts(false));
        let on = verify(&candidate, &opts(true));
        assert_eq!(summary(&off), summary(&on), "backend divergence");
        assert_eq!(off.is_ok(), on.is_ok());
    }
}
