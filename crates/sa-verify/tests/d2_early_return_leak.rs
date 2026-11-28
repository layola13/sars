//! D2: early_return scans for live allocations and reports EarlyReturnLeak
//! (sci armTry / early_return_leak, trap code 1026).
use sa_ir::{
    Block, Diagnostic, Function, Instruction, InstructionKind, Operand, Program, Signature,
    SourceLoc, Trap,
};
use sa_verify::{verify, VerifyOptions};

fn loc(line: u32) -> SourceLoc {
    SourceLoc::new("test.sa", line, 1)
}

fn ins(kind: InstructionKind, line: u32) -> Instruction {
    Instruction::new(kind, loc(line), "")
}

fn program(instrs: Vec<Instruction>) -> Program {
    let mut b = Block::new("L_entry", loc(1));
    b.instrs = instrs;
    let mut f = Function::new(Signature::new("main"), loc(1));
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

#[test]
fn d2_early_return_with_live_allocation_is_leak() {
    let mut alloc = ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2);
    alloc.dest = Some("p".into());
    let d = check(&program(vec![
        alloc,
        ins(InstructionKind::EarlyReturn { value: None }, 3),
    ]));
    assert!(d.iter().any(|x| x.trap == Trap::EarlyReturnLeak), "{d:?}");
}

#[test]
fn d2_early_return_after_release_is_clean() {
    let mut alloc = ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2);
    alloc.dest = Some("p".into());
    let d = check(&program(vec![
        alloc,
        ins(InstructionKind::Release { reg: "p".into() }, 3),
        ins(InstructionKind::EarlyReturn { value: None }, 4),
    ]));
    assert!(!d.iter().any(|x| x.trap == Trap::EarlyReturnLeak), "{d:?}");
}

#[test]
fn d2_normal_return_still_reports_memory_leak() {
    // The ordinary return path keeps its distinct MemoryLeak trap.
    let mut alloc = ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2);
    alloc.dest = Some("p".into());
    let d = check(&program(vec![alloc, ins(InstructionKind::Return { value: None }, 3)]));
    assert!(d.iter().any(|x| x.trap == Trap::MemoryLeak), "{d:?}");
    assert!(!d.iter().any(|x| x.trap == Trap::EarlyReturnLeak), "{d:?}");
}
