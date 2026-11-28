//! B2: raw pointer / assume_* instructions are legal only inside @ffi_wrapper
//! (sci armRawCast/armAssumeSafe/armAssumeBorrow → illegal_unsafe_context).
use sa_ir::{
    Block, Diagnostic, Function, FunctionKind, Instruction, InstructionKind, Operand, Program,
    Signature, SourceLoc, Trap,
};
use sa_verify::{verify, VerifyOptions};

fn loc(line: u32) -> SourceLoc {
    SourceLoc::new("test.sa", line, 1)
}

fn ins(kind: InstructionKind, line: u32) -> Instruction {
    Instruction::new(kind, loc(line), "")
}

fn program(kind: FunctionKind, unsafe_ins: Instruction) -> Program {
    let mut sig = Signature::new("f");
    sig.kind = kind;
    let mut b = Block::new("L_entry", loc(1));
    // Allocate a source register so the unsafe instruction has a live operand.
    let mut alloc = ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2);
    alloc.dest = Some("p".into());
    let mut unsafe_ins = unsafe_ins;
    if unsafe_ins.dest.is_none() {
        unsafe_ins.dest = Some("u".into());
    }
    b.instrs = vec![alloc, unsafe_ins, ins(InstructionKind::Return { value: None }, 4)];
    let mut f = Function::new(sig, loc(1));
    f.blocks = vec![b];
    let mut p = Program::new();
    p.functions.push(f);
    p
}

fn check(p: &Program) -> Vec<Diagnostic> {
    let mut all = Vec::new();
    for use_regstate in [false, true] {
        let opts = VerifyOptions { use_regstate, ..VerifyOptions::default() };
        if let Err(diags) = verify(p, &opts) {
            all.extend(diags);
        }
    }
    all
}

fn has_unsafe(d: &[Diagnostic]) -> bool {
    d.iter().any(|x| x.trap == Trap::IllegalUnsafeContext)
}

#[test]
fn b2_assume_safe_outside_ffi_wrapper_rejected() {
    let unsafe_ins = ins(InstructionKind::AssumeSafe { src: Operand::Reg("p".into()) }, 3);
    let d = check(&program(FunctionKind::Normal, unsafe_ins));
    assert!(has_unsafe(&d), "{d:?}");
}

#[test]
fn b2_raw_cast_outside_ffi_wrapper_rejected() {
    let unsafe_ins = ins(InstructionKind::RawCast { src: Operand::Reg("p".into()) }, 3);
    let d = check(&program(FunctionKind::Normal, unsafe_ins));
    assert!(has_unsafe(&d), "{d:?}");
}

#[test]
fn b2_assume_borrow_outside_ffi_wrapper_rejected() {
    let unsafe_ins = ins(
        InstructionKind::AssumeBorrow { src: Operand::Reg("p".into()), mutable: false },
        3,
    );
    let d = check(&program(FunctionKind::Normal, unsafe_ins));
    assert!(has_unsafe(&d), "{d:?}");
}

#[test]
fn b2_unsafe_inside_ffi_wrapper_allowed() {
    for kind in [
        InstructionKind::AssumeSafe { src: Operand::Reg("p".into()) },
        InstructionKind::RawCast { src: Operand::Reg("p".into()) },
        InstructionKind::AssumeBorrow { src: Operand::Reg("p".into()), mutable: false },
    ] {
        let p = program(FunctionKind::FfiWrapper, ins(kind, 3));
        let d = check(&p);
        assert!(!has_unsafe(&d), "{d:?}");
    }
}

#[test]
fn b2_ordinary_borrow_outside_ffi_wrapper_allowed() {
    // A normal shared borrow is not unsafe and must remain legal.
    let mut b = Block::new("L_entry", loc(1));
    let mut alloc = ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2);
    alloc.dest = Some("p".into());
    let mut borrow = ins(InstructionKind::Borrow { src: Operand::Reg("p".into()), mutable: false }, 3);
    borrow.dest = Some("v".into());
    b.instrs = vec![alloc, borrow, ins(InstructionKind::Return { value: None }, 4)];
    let mut f = Function::new(Signature::new("f"), loc(1));
    f.blocks = vec![b];
    let mut p = Program::new();
    p.functions.push(f);
    let d = check(&p);
    assert!(!has_unsafe(&d), "{d:?}");
}
