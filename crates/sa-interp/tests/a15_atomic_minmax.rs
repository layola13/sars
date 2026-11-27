//! A15: atomic_rmw min/max are reductions returning an operand, not comparisons.
//! sci `atomicRmwApplyImpl` (`interpreter_init.zig:383-405`).
use sa_interp::{Interpreter, InterpOptions, Value};
use sa_ir::{
    Address, AtomicOrdering, AtomicRmwOp, Block, Function, Instruction, InstructionKind, Operand,
    PrimType, Program, Signature, SourceLoc,
};

fn ins(kind: InstructionKind) -> Instruction {
    Instruction::new(kind, SourceLoc::unknown(), "test")
}

fn run_rmw(op: AtomicRmwOp, initial: i64, rhs: i64, ty: PrimType) -> i128 {
    let mut rmw = ins(InstructionKind::AtomicRmw {
        op,
        addr: Address::new(Operand::reg("p"), 0),
        value: Operand::Int(rhs as i128),
        order: AtomicOrdering::SeqCst,
    });
    rmw.dest = Some("old".into());
    rmw.ty = Some(ty);
    let mut load = ins(InstructionKind::Load {
        addr: Address::new(Operand::reg("p"), 0),
    });
    load.dest = Some("result".into());
    load.ty = Some(ty);
    let mut ret = ins(InstructionKind::Return {
        value: Some(Operand::reg("result")),
    });
    ret.ty = Some(ty);
    let mut store = ins(InstructionKind::Store {
        addr: Address::new(Operand::reg("p"), 0),
        value: Operand::Int(initial as i128),
    });
    store.ty = Some(ty);
    let mut b = Block::new("entry", SourceLoc::unknown());
    b.instrs = vec![
        ins(InstructionKind::Alloc { size: Operand::Int(8) }).with_dest("p"),
        store,
        rmw,
        load,
        ret,
    ];
    let mut sig = Signature::new("main");
    sig.return_ty = ty;
    let mut f = Function::new(sig, SourceLoc::unknown());
    f.blocks = vec![b];
    let p = Program { functions: vec![f], ..Program::default() };
    let mut it = Interpreter::new(&p, InterpOptions::default());
    match it.run_function("main", Vec::new()).unwrap() {
        Value::Int(v) => v,
        other => panic!("expected Int, got {other:?}"),
    }
}

#[test]
fn a15_umin_returns_operand_not_boolean() {
    // -1 as u32 = 4294967295, so umin(-1, 5) = 5; a comparison would store 0.
    assert_eq!(run_rmw(AtomicRmwOp::Umin, -1, 5, PrimType::U32), 5);
    assert_eq!(run_rmw(AtomicRmwOp::Umin, 10, 3, PrimType::U32), 3);
    assert_eq!(run_rmw(AtomicRmwOp::Umin, 3, 10, PrimType::U32), 3);
}

#[test]
fn a15_umax_returns_operand_not_boolean() {
    assert_eq!(run_rmw(AtomicRmwOp::Umax, 10, 3, PrimType::U32), 10);
    // -1 as u32 = 4294967295; unsigned load keeps the unsigned representation.
    assert_eq!(run_rmw(AtomicRmwOp::Umax, -1, 5, PrimType::U32), 4294967295);
}

#[test]
fn a15_smin_smax_signed_comparison() {
    assert_eq!(run_rmw(AtomicRmwOp::Smin, -5, 2, PrimType::I32), -5);
    assert_eq!(run_rmw(AtomicRmwOp::Smin, 2, -5, PrimType::I32), -5);
    assert_eq!(run_rmw(AtomicRmwOp::Smax, -5, 2, PrimType::I32), 2);
    assert_eq!(run_rmw(AtomicRmwOp::Smax, 2, -5, PrimType::I32), 2);
}

#[test]
fn a15_minmax_respects_width_mask() {
    assert_eq!(run_rmw(AtomicRmwOp::Smin, -1, 0x7FFF, PrimType::I16), -1);
    assert_eq!(run_rmw(AtomicRmwOp::Umax, -1, 1, PrimType::U16), 65535);
}
