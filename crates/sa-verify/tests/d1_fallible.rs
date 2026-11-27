//! D1: fallible contract — `?` requires a fallible source and fallible values
//! may not be returned from a non-fallible function (sci armTry/armReturn).
use sa_ir::{
    Block, Diagnostic, Function, Instruction, InstructionKind, Operand, PrimType, Program,
    Signature, SourceLoc, Trap,
};
use sa_verify::{verify, VerifyOptions};

fn loc(line: u32) -> SourceLoc {
    SourceLoc::new("test.sa", line, 1)
}

fn ins(kind: InstructionKind, line: u32) -> Instruction {
    Instruction::new(kind, loc(line), "")
}

fn block(label: &str, instrs: Vec<Instruction>, line: u32) -> Block {
    let mut b = Block::new(label, loc(line));
    b.instrs = instrs;
    b
}

fn fallible_program(instrs: Vec<Instruction>, fn_fallible: bool) -> Program {
    let mut ext = Signature::new("fallible");
    ext.return_fallible = true;
    ext.return_ty = PrimType::Void;
    let mut sig = Signature::new("main");
    sig.return_fallible = fn_fallible;
    sig.return_ty = PrimType::Void;
    let mut f = Function::new(sig, loc(1));
    f.blocks = vec![block("L_entry", instrs, 2)];
    let mut p = Program::new();
    p.externs.push(ext);
    p.functions.push(f);
    p
}

fn has(diags: &[Diagnostic], trap: Trap) -> bool {
    diags.iter().any(|d| d.trap == trap)
}

fn check(p: &Program, regstate: bool) -> Vec<Diagnostic> {
    let opts = VerifyOptions { use_regstate: regstate, ..VerifyOptions::default() };
    verify(p, &opts).err().unwrap_or_default()
}

#[test]
fn d1_try_rejects_non_fallible_source() {
    for rs in [false, true] {
        let p = fallible_program(
            vec![
                ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("a"),
                ins(InstructionKind::Try { src: Operand::Reg("a".into()) }, 3).with_dest("u"),
                ins(InstructionKind::Release { reg: "a".into() }, 4),
                ins(InstructionKind::Return { value: None }, 5),
            ],
            false,
        );
        let diags = check(&p, rs);
        assert!(has(&diags, Trap::FallibleContractMismatch), "rs={rs}: {diags:?}");
    }
}

#[test]
fn d1_try_accepts_fallible_call_result() {
    for rs in [false, true] {
        let p = fallible_program(
            vec![
                ins(InstructionKind::Call { target: "fallible".into(), args: vec![] }, 2)
                    .with_dest("r"),
                ins(InstructionKind::Try { src: Operand::Reg("r".into()) }, 3).with_dest("u"),
                ins(InstructionKind::Return { value: None }, 4),
            ],
            false,
        );
        let diags = check(&p, rs);
        assert!(!has(&diags, Trap::FallibleContractMismatch), "rs={rs}: {diags:?}");
    }
}

#[test]
fn d1_return_rejects_fallible_from_nonfallible_function() {
    for rs in [false, true] {
        let p = fallible_program(
            vec![
                ins(InstructionKind::Call { target: "fallible".into(), args: vec![] }, 2)
                    .with_dest("r"),
                ins(InstructionKind::Return { value: Some(Operand::Reg("r".into())) }, 3),
            ],
            false,
        );
        let diags = check(&p, rs);
        assert!(has(&diags, Trap::FallibleContractMismatch), "rs={rs}: {diags:?}");
    }
}

#[test]
fn d1_return_accepts_fallible_from_fallible_function() {
    for rs in [false, true] {
        let p = fallible_program(
            vec![
                ins(InstructionKind::Call { target: "fallible".into(), args: vec![] }, 2)
                    .with_dest("r"),
                ins(InstructionKind::Return { value: Some(Operand::Reg("r".into())) }, 3),
            ],
            true,
        );
        let diags = check(&p, rs);
        assert!(!has(&diags, Trap::FallibleContractMismatch), "rs={rs}: {diags:?}");
    }
}
