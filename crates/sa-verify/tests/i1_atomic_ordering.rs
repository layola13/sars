//! I1: same-address RMW ordering history keyed by (base, offset), with sci
//! `sameAddressRmwCompatible` (equal or seq_cst), cmpxchg included.
use sa_ir::{
    Address, AtomicOrdering, AtomicRmwOp, Block, Diagnostic, Function, Instruction,
    InstructionKind, Operand, PrimType, Program, Signature, SourceLoc, Trap,
};
use sa_verify::{verify, VerifyOptions};

fn loc(line: u32) -> SourceLoc {
    SourceLoc::new("test.sa", line, 1)
}

fn ins(kind: InstructionKind, line: u32) -> Instruction {
    Instruction::new(kind, loc(line), "")
}

fn rmw(addr: Address, order: AtomicOrdering, line: u32) -> Instruction {
    let mut i = ins(
        InstructionKind::AtomicRmw {
            op: AtomicRmwOp::Add,
            addr,
            value: Operand::Int(1),
            order,
        },
        line,
    );
    i.dest = Some(format!("old{line}"));
    i
}

fn check(instrs: Vec<Instruction>) -> Vec<Diagnostic> {
    let mut b = Block::new("L_entry", loc(1));
    b.instrs = instrs;
    let mut f = Function::new(Signature::new("main"), loc(1));
    f.blocks = vec![b];
    let mut p = Program::new();
    p.functions.push(f);
    let mut all = Vec::new();
    for use_regstate in [false, true] {
        let opts = VerifyOptions { use_regstate, ..VerifyOptions::default() };
        if let Err(diags) = verify(&p, &opts) {
            all.extend(diags);
        }
    }
    all
}

fn has_ordering_mismatch(d: &[Diagnostic]) -> bool {
    d.iter().any(|x| x.trap == Trap::AtomicOrderingMismatch)
}

#[test]
fn i1_same_address_seq_cst_mixes_with_relaxed() {
    // sci: seq_cst is compatible with every ordering; the old sars `!=` check
    // reported a mismatch for this valid sequence.
    let d = check(vec![
        ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("p"),
        rmw(Address::new(Operand::reg("p"), 0), AtomicOrdering::Relaxed, 3),
        rmw(Address::new(Operand::reg("p"), 0), AtomicOrdering::SeqCst, 4),
        ins(InstructionKind::Return { value: None }, 5),
    ]);
    assert!(!has_ordering_mismatch(&d), "{d:?}");
}

#[test]
fn i1_same_address_relaxed_then_acquire_is_mismatch() {
    let d = check(vec![
        ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("p"),
        rmw(Address::new(Operand::reg("p"), 0), AtomicOrdering::Relaxed, 3),
        rmw(Address::new(Operand::reg("p"), 0), AtomicOrdering::Acquire, 4),
        ins(InstructionKind::Return { value: None }, 5),
    ]);
    assert!(has_ordering_mismatch(&d), "{d:?}");
}

#[test]
fn i1_distinct_offsets_are_distinct_addresses() {
    // Same base, different static offset: sci keys on (reg, offset), so these
    // must not be compared at all.
    let d = check(vec![
        ins(InstructionKind::Alloc { size: Operand::Int(16) }, 2).with_dest("p"),
        rmw(Address::new(Operand::reg("p"), 0), AtomicOrdering::Relaxed, 3),
        rmw(Address::new(Operand::reg("p"), 8), AtomicOrdering::Acquire, 4),
        ins(InstructionKind::Return { value: None }, 5),
    ]);
    assert!(!has_ordering_mismatch(&d), "{d:?}");
}

#[test]
fn i1_cmpxchg_participates_in_ordering_history() {
    let mut cmpxchg = ins(
        InstructionKind::Cmpxchg {
            addr: Address::new(Operand::reg("p"), 0),
            expected: Operand::Int(0),
            replacement: Operand::Int(1),
            order: AtomicOrdering::Acquire,
        },
        4,
    );
    cmpxchg.dest = Some("old".into());
    cmpxchg.extra_dests = vec!["ok".into()];
    let d = check(vec![
        ins(InstructionKind::Alloc { size: Operand::Int(8) }, 2).with_dest("p"),
        rmw(Address::new(Operand::reg("p"), 0), AtomicOrdering::Relaxed, 3),
        cmpxchg,
        ins(InstructionKind::Return { value: None }, 5),
    ]);
    assert!(has_ordering_mismatch(&d), "{d:?}");
}
