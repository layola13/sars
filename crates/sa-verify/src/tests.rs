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
