use super::*;
use sa_ir::{
    Block, CapPrefix, Function, Instruction, InstructionKind, Operand, ParamSpec, PrimType,
    Signature, SourceLoc,
};

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

fn has_trap(errors: &[Diagnostic], trap: Trap) -> bool {
    errors.iter().any(|diagnostic| diagnostic.trap == trap)
}

#[test]
fn valid_program_returns_ok() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                ins(InstructionKind::Release { reg: "a".into() }, 3),
                ins(InstructionKind::Return { value: None }, 4),
            ],
            1,
        )],
    );
    let program = program(vec![source]);
    let report = verify(&program, &VerifyOptions::default()).expect("valid program");
    assert_eq!(report.functions_checked, 1);
    assert_eq!(report.instructions_checked, 3);
    assert!(report.warnings.is_empty());
}

#[test]
fn use_after_move_is_rejected() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                ins(InstructionKind::Move { src: Operand::reg("a") }, 3).with_dest("b"),
                ins(
                    InstructionKind::Op {
                        op: sa_ir::OpKind::Add,
                        args: vec![Operand::reg("a"), Operand::Int(1)],
                    },
                    4,
                )
                .with_dest("c"),
                ins(InstructionKind::Return { value: None }, 5),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::UseAfterMove), "{errors:?}");
    assert!(errors[0].register.as_deref() == Some("a"));
    assert!(errors[0].function.as_deref() == Some("main"));
}

#[test]
fn double_mutable_borrow_is_rejected() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                ins(InstructionKind::Borrow { src: Operand::reg("a"), mutable: true }, 3).with_dest("b"),
                ins(InstructionKind::Borrow { src: Operand::reg("a"), mutable: true }, 4).with_dest("c"),
                ins(InstructionKind::Return { value: None }, 5),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::DoubleMutableBorrow), "{errors:?}");
}

#[test]
fn shared_then_mutable_borrow_is_rejected() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                ins(InstructionKind::Borrow { src: Operand::reg("a"), mutable: false }, 3).with_dest("b"),
                ins(InstructionKind::Borrow { src: Operand::reg("a"), mutable: true }, 4).with_dest("c"),
                ins(InstructionKind::Return { value: None }, 5),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::BorrowConflict), "{errors:?}");
}

#[test]
fn memory_leak_is_reported_with_hint() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::MemoryLeak), "{errors:?}");
    assert!(errors.iter().any(|diagnostic| diagnostic
        .hint
        .as_deref()
        .map(|hint| hint.contains("!a"))
        .unwrap_or(false)));
}

#[test]
fn returned_allocation_is_not_a_leak() {
    let mut signature = Signature::new("main");
    signature.return_ty = PrimType::Ptr;
    let mut source = Function::new(signature, loc(1));
    source.blocks.push(block(
        "L_entry",
        vec![
            ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
            ins(InstructionKind::Return { value: Some(Operand::reg("a")) }, 3),
        ],
        1,
    ));
    verify(&program(vec![source]), &VerifyOptions::default()).expect("no leak");
}

#[test]
fn unknown_register_is_rejected() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(
                    InstructionKind::Op {
                        op: sa_ir::OpKind::Add,
                        args: vec![Operand::reg("missing"), Operand::Int(1)],
                    },
                    2,
                )
                .with_dest("c"),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::UnknownRegister), "{errors:?}");
}

#[test]
fn duplicate_label_is_rejected() {
    let source = function(
        "main",
        vec![
            block("L_same", vec![ins(InstructionKind::Jmp { target: "L_same".into() }, 2)], 1),
            block("L_same", vec![ins(InstructionKind::Return { value: None }, 4)], 3),
        ],
    );
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::DuplicateLabel), "{errors:?}");
}

#[test]
fn jump_to_unknown_label_is_rejected() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Jmp { target: "L_missing".into() }, 2),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::UnknownRegister), "{errors:?}");
    assert!(errors.iter().any(|diagnostic| diagnostic.message.contains("label")));
}

#[test]
fn capability_mismatch_is_rejected() {
    let mut signature = Signature::new("helper");
    signature.params.push(ParamSpec::new("p", PrimType::I64, CapPrefix::ByValue));
    let mut helper = Function::new(signature, loc(1));
    helper.blocks.push(block(
        "L_entry",
        vec![ins(InstructionKind::Return { value: None }, 2)],
        1,
    ));
    let main = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                ins(
                    InstructionKind::Call {
                        target: "helper".into(),
                        args: vec![sa_ir::CallArg::new(CapPrefix::Raw, Operand::reg("a"))],
                    },
                    3,
                ),
                ins(InstructionKind::Release { reg: "a".into() }, 4),
                ins(InstructionKind::Return { value: None }, 5),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![helper, main]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::CapabilityMismatch), "{errors:?}");
}

#[test]
fn missing_call_target_is_rejected() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Call { target: "ghost".into(), args: Vec::new() }, 2),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::UnknownRegister), "{errors:?}");
}

#[test]
fn gas_limit_is_enforced() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                ins(InstructionKind::Release { reg: "a".into() }, 3),
                ins(InstructionKind::Return { value: None }, 4),
            ],
            1,
        )],
    );
    let options = VerifyOptions { max_gas: 2, ..VerifyOptions::default() };
    let errors = verify(&program(vec![source]), &options).unwrap_err();
    assert!(has_trap(&errors, Trap::GasExceeded), "{errors:?}");
}

#[test]
fn phi_conflict_is_a_warning() {
    let mut source = function(
        "main",
        vec![
            block(
                "L_entry",
                vec![
                    ins(
                        InstructionKind::Br {
                            cond: Operand::reg("c"),
                            then_label: "L_left".into(),
                            else_label: "L_right".into(),
                        },
                        2,
                    ),
                ],
                1,
            ),
            block(
                "L_left",
                vec![
                    ins(InstructionKind::Alloc { size: Operand::Int(8) }, 4).with_dest("a"),
                    ins(InstructionKind::Jmp { target: "L_join".into() }, 5),
                ],
                3,
            ),
            block(
                "L_right",
                vec![
                    ins(InstructionKind::Move { src: Operand::reg("p") }, 7).with_dest("a"),
                    ins(InstructionKind::Jmp { target: "L_join".into() }, 8),
                ],
                6,
            ),
            block(
                "L_join",
                vec![ins(InstructionKind::Return { value: None }, 10)],
                9,
            ),
        ],
    );
    source.locals.push("c".into());
    source.sig.params.push(ParamSpec::new("p", PrimType::I64, CapPrefix::ByValue));
    let report = verify(&program(vec![source]), &VerifyOptions::default()).expect("warning only");
    assert!(report.warnings.iter().any(|diagnostic| diagnostic.trap == Trap::PhiStateConflict));
}

#[test]
fn fallthrough_is_a_warning() {
    let source = function(
        "main",
        vec![
            block("L_entry", vec![ins(InstructionKind::Nop, 2)], 1),
            block("L_next", vec![ins(InstructionKind::Return { value: None }, 4)], 3),
        ],
    );
    let report = verify(&program(vec![source]), &VerifyOptions::default()).expect("warning only");
    assert!(report.warnings.iter().any(|diagnostic| diagnostic.trap == Trap::FallthroughForbidden));
}

#[test]
fn fallible_contract_mismatch_is_rejected() {
    let mut signature = Signature::new("plain");
    signature.return_ty = PrimType::I32;
    let mut helper = Function::new(signature, loc(1));
    helper.blocks.push(block(
        "L_entry",
        vec![ins(InstructionKind::Return { value: Some(Operand::Int(1)) }, 2)],
        1,
    ));
    let main = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(
                    InstructionKind::Call { target: "plain".into(), args: Vec::new() },
                    2,
                )
                .with_dest("r"),
                ins(InstructionKind::Try { src: Operand::reg("r") }, 3).with_dest("v"),
                ins(InstructionKind::Return { value: None }, 4),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![helper, main]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::FallibleContractMismatch), "{errors:?}");
}

#[test]
fn extra_destinations_are_defined_by_their_instruction() {
    let mut exchange = ins(
        InstructionKind::Cmpxchg {
            addr: sa_ir::Address::new(Operand::reg("p"), 0),
            expected: Operand::Int(0),
            replacement: Operand::Int(1),
            order: sa_ir::AtomicOrdering::SeqCst,
        },
        3,
    )
    .with_dest("old");
    exchange.extra_dests = vec!["ok".into()];
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("p"),
                exchange,
                ins(InstructionKind::Release { reg: "old".into() }, 4),
                ins(InstructionKind::Release { reg: "ok".into() }, 5),
                ins(InstructionKind::Release { reg: "p".into() }, 6),
                ins(InstructionKind::Return { value: None }, 7),
            ],
            1,
        )],
    );
    verify(&program(vec![source]), &VerifyOptions::default()).expect("extra dests are defined");
}

#[test]
fn unused_allocation_is_still_a_leak() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("p"),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::MemoryLeak), "{errors:?}");
}

#[test]
fn allocation_stored_into_memory_is_not_a_leak() {
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::StackAlloc { size: Operand::Int(8) }, 2).with_dest("slot"),
                ins(
                    InstructionKind::Store {
                        addr: sa_ir::Address::new(Operand::reg("slot"), 0),
                        value: Operand::Int(1),
                    },
                    3,
                ),
                ins(InstructionKind::Return { value: None }, 4),
            ],
            1,
        )],
    );
    verify(&program(vec![source]), &VerifyOptions::default()).expect("stored allocation escaped");
}

#[test]
fn allocation_passed_to_call_is_not_a_leak() {
    let mut sink = function("sink", vec![block("L_entry", vec![ins(InstructionKind::Return { value: None }, 2)], 1)]);
    sink.sig.params.push(sa_ir::ParamSpec::new("p", PrimType::Ptr, CapPrefix::Borrow));
    let source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("p"),
                ins(
                    InstructionKind::Call {
                        target: "sink".into(),
                        args: vec![sa_ir::CallArg::new(CapPrefix::Borrow, Operand::reg("p"))],
                    },
                    3,
                ),
                ins(InstructionKind::Return { value: None }, 4),
            ],
            1,
        )],
    );
    verify(&program(vec![source, sink]), &VerifyOptions::default()).expect("passed allocation escaped");
}

#[test]
fn plain_assignment_of_param_keeps_source_usable() {
    let mut source = function(
        "main",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Assign { src: Operand::reg("p") }, 2).with_dest("q"),
                ins(
                    InstructionKind::Call {
                        target: "sink".into(),
                        args: vec![sa_ir::CallArg::new(CapPrefix::Raw, Operand::reg("p"))],
                    },
                    3,
                ),
                ins(InstructionKind::Release { reg: "q".into() }, 4),
                ins(InstructionKind::Return { value: None }, 5),
            ],
            1,
        )],
    );
    source.sig.params.push(sa_ir::ParamSpec::new("p", PrimType::Ptr, CapPrefix::Raw));
    let mut sink = function("sink", vec![block("L_entry", vec![ins(InstructionKind::Return { value: None }, 2)], 1)]);
    sink.sig.params.push(sa_ir::ParamSpec::new("p", PrimType::Ptr, CapPrefix::Raw));
    verify(&program(vec![source, sink]), &VerifyOptions::default())
        .expect("assigning a param is a copy");
}

#[test]
fn live_borrow_param_in_indirect_dispatch_is_a_leak() {
    let mut source = function(
        "dispatch",
        vec![block(
            "L_entry",
            vec![
                ins(
                    InstructionKind::CallIndirect {
                        target: Operand::Symbol("vt".into()),
                        args: vec![sa_ir::CallArg::new(CapPrefix::Borrow, Operand::reg("slot"))],
                    },
                    2,
                ),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    source
        .sig
        .params
        .push(ParamSpec::new("slot", PrimType::Ptr, CapPrefix::Borrow));
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::MemoryLeak), "{errors:?}");
}

#[test]
fn live_borrow_param_without_indirect_dispatch_is_not_a_leak() {
    let mut source = function(
        "reads",
        vec![block("L_entry", vec![ins(InstructionKind::Return { value: None }, 2)], 1)],
    );
    source
        .sig
        .params
        .push(ParamSpec::new("slot", PrimType::Ptr, CapPrefix::Borrow));
    verify(&program(vec![source]), &VerifyOptions::default()).expect("borrow param is not owned");
}

#[test]
fn unreleased_owned_ptr_param_is_an_exit_leak() {
    let mut source = function(
        "consume",
        vec![block("L_entry", vec![ins(InstructionKind::Return { value: None }, 2)], 1)],
    );
    source
        .sig
        .params
        .push(ParamSpec::new("handle", PrimType::Ptr, CapPrefix::Move));
    let errors = verify(&program(vec![source]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::MemoryLeak), "{errors:?}");
    assert!(errors.iter().any(|diagnostic| diagnostic.register.as_deref() == Some("handle")));
}

#[test]
fn released_owned_ptr_param_is_not_an_exit_leak() {
    let mut source = function(
        "consume",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Release { reg: "handle".into() }, 2),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    source
        .sig
        .params
        .push(ParamSpec::new("handle", PrimType::Ptr, CapPrefix::ByValue));
    verify(&program(vec![source]), &VerifyOptions::default()).expect("released param is not a leak");
}

#[test]
fn moved_owned_ptr_param_is_not_an_exit_leak() {
    let mut source = function(
        "consume",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Move { src: Operand::reg("handle") }, 2).with_dest("moved"),
                ins(InstructionKind::Release { reg: "moved".into() }, 3),
                ins(InstructionKind::Return { value: None }, 4),
            ],
            1,
        )],
    );
    source
        .sig
        .params
        .push(ParamSpec::new("handle", PrimType::Ptr, CapPrefix::Move));
    verify(&program(vec![source]), &VerifyOptions::default()).expect("moved param is not a leak");
}

#[test]
fn live_scalar_param_is_not_an_exit_leak() {
    let mut source = function(
        "consume",
        vec![block("L_entry", vec![ins(InstructionKind::Return { value: None }, 2)], 1)],
    );
    source
        .sig
        .params
        .push(ParamSpec::new("n", PrimType::I32, CapPrefix::ByValue));
    verify(&program(vec![source]), &VerifyOptions::default()).expect("scalars cannot leak");
}

#[test]
fn verify_function_checks_only_one_function() {
    let main = function(
        "main",
        vec![block("L_entry", vec![ins(InstructionKind::Return { value: None }, 2)], 1)],
    );
    let other = function(
        "other",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    let program = program(vec![main.clone(), other]);
    let report = verify_function(&main, &program, &VerifyOptions::default()).expect("main is valid");
    assert_eq!(report.functions_checked, 1);
    assert_eq!(report.instructions_checked, 1);
}

#[test]
fn borrow_arg_to_by_value_ptr_param_is_accepted() {
    let mut callee = function(
        "log_emit",
        vec![block(
            "L_entry",
            vec![
                ins(InstructionKind::Release { reg: "msg".into() }, 2),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    callee
        .sig
        .params
        .push(ParamSpec::new("msg", PrimType::Ptr, CapPrefix::ByValue));
    let mut caller = function(
        "consumer",
        vec![block(
            "L_entry",
            vec![
                ins(
                    InstructionKind::Call {
                        target: "log_emit".into(),
                        args: vec![sa_ir::CallArg::new(CapPrefix::Borrow, Operand::reg("MSG"))],
                    },
                    2,
                ),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    caller
        .sig
        .params
        .push(ParamSpec::new("MSG", PrimType::Ptr, CapPrefix::Borrow));
    verify(&program(vec![callee, caller]), &VerifyOptions::default())
        .expect("borrow arg to by-value ptr param mirrors sci callPrefixMatchesParam (257)");
}

#[test]
fn borrow_arg_to_by_value_scalar_param_is_rejected() {
    let mut callee = function(
        "link_target",
        vec![block("L_entry", vec![ins(InstructionKind::Return { value: None }, 2)], 1)],
    );
    callee
        .sig
        .params
        .push(ParamSpec::new("slot", PrimType::I32, CapPrefix::ByValue));
    let mut caller = function(
        "consumer",
        vec![block(
            "L_entry",
            vec![
                ins(
                    InstructionKind::Call {
                        target: "link_target".into(),
                        args: vec![sa_ir::CallArg::new(CapPrefix::Borrow, Operand::reg("slot"))],
                    },
                    2,
                ),
                ins(InstructionKind::Return { value: None }, 3),
            ],
            1,
        )],
    );
    caller
        .sig
        .params
        .push(ParamSpec::new("slot", PrimType::Ptr, CapPrefix::Borrow));
    let errors = verify(&program(vec![callee, caller]), &VerifyOptions::default()).unwrap_err();
    assert!(has_trap(&errors, Trap::CapabilityMismatch), "{errors:?}");
}
