use crate::{emit_c, COptions};
use sa_ir::{
    AtomicOrdering, AtomicRmwOp, Block, CallArg, CapPrefix, ConstDecl, ConstValue, Function,
    Instruction, InstructionKind, NpmBindDecl, NpmBindShape, OpKind, Operand, ParamSpec, PrimType,
    Program, Signature, SourceLoc, Trap,
};

fn loc() -> SourceLoc {
    SourceLoc::unknown()
}

fn print_extern() -> sa_ir::Signature {
    let mut sig = Signature::new("sa_print_bytes");
    sig.params.push(ParamSpec::new("msg", PrimType::Ptr, CapPrefix::Borrow));
    sig.params.push(ParamSpec::new("len", PrimType::U64, CapPrefix::ByValue));
    sig
}

fn hello_program() -> Program {
    let mut p = Program::new();
    p.consts.push(ConstDecl::new(
        "MSG",
        ConstValue::Utf8("hi\n".to_string()),
    ));
    p.externs.push(print_extern());
    let mut f = Function::new(Signature::new("main"), loc());
    let mut b = Block::new("L_ENTRY", loc());
    b.instrs.push(Instruction::new(
        InstructionKind::Call {
            target: "sa_print_bytes".to_string(),
            args: vec![
                CallArg::new(CapPrefix::Borrow, Operand::reg("MSG")),
                CallArg::new(CapPrefix::ByValue, Operand::Int(3)),
            ],
        },
        loc(),
        "call",
    ));
    b.instrs.push(Instruction::new(
        InstructionKind::Return {
            value: Some(Operand::Int(0)),
        },
        loc(),
        "return",
    ));
    f.blocks.push(b);
    p.functions.push(f);
    p
}

#[test]
fn emits_hello_world_c() {
    let c = emit_c(&hello_program(), &COptions::default()).unwrap();
    assert!(c.contains("static const unsigned char sa_c0_MSG[]"));
    assert!(c.contains("sa_print_bytes(sa_c0_MSG, (uint64_t)3);"));
    assert!(c.contains("int main(void)"));
    assert!(c.contains("static void sa_print_bytes(const uint8_t *p, uint64_t len)"));
}

#[test]
fn emits_alloc_store_load_add() {
    let mut p = Program::new();
    let mut f = Function::new(Signature::new("main"), loc());
    let mut b = Block::new("L_ENTRY", loc());
    b.instrs.push(Instruction::new(
        InstructionKind::Alloc {
            size: Operand::Int(8),
        },
        loc(),
        "alloc",
    ).with_dest("arr"));
    b.instrs.push(Instruction::new(
        InstructionKind::Store {
            addr: sa_ir::Address::new(Operand::reg("arr"), 0),
            value: Operand::Int(3),
        },
        loc(),
        "store",
    ).with_ty(PrimType::I32));
    b.instrs.push(Instruction::new(
        InstructionKind::Load {
            addr: sa_ir::Address::new(Operand::reg("arr"), 0),
        },
        loc(),
        "load",
    ).with_dest("x").with_ty(PrimType::I32));
    b.instrs.push(Instruction::new(
        InstructionKind::Op {
            op: OpKind::Add,
            args: vec![Operand::reg("x"), Operand::Int(4)],
        },
        loc(),
        "add",
    ).with_dest("y"));
    b.instrs.push(Instruction::new(
        InstructionKind::Return {
            value: Some(Operand::reg("y")),
        },
        loc(),
        "return",
    ));
    f.blocks.push(b);
    p.functions.push(f);
    let c = emit_c(&p, &COptions::default()).unwrap();
    assert!(c.contains("sa_alloc((uint64_t)("));
    assert!(c.contains("*(int32_t *)"));
    assert!(c.contains("+ ((int32_t)4)"));
}

#[test]
fn emits_atomics_and_fences() {
    let mut p = Program::new();
    let mut f = Function::new(Signature::new("main"), loc());
    let mut b = Block::new("L_ENTRY", loc());
    let addr = sa_ir::Address::new(Operand::reg("p"), 0);
    b.instrs.push(Instruction::new(
        InstructionKind::Alloc { size: Operand::Int(8) },
        loc(),
        "alloc",
    ).with_dest("p"));
    b.instrs.push(Instruction::new(
        InstructionKind::AtomicLoad {
            addr: addr.clone(),
            order: AtomicOrdering::Acquire,
        },
        loc(),
        "atomic_load",
    ).with_dest("v").with_ty(PrimType::I32));
    b.instrs.push(Instruction::new(
        InstructionKind::AtomicStore {
            addr: addr.clone(),
            value: Operand::Int(7),
            order: AtomicOrdering::Release,
        },
        loc(),
        "atomic_store",
    ).with_ty(PrimType::I32));
    b.instrs.push(Instruction::new(
        InstructionKind::AtomicRmw {
            op: AtomicRmwOp::Add,
            addr: addr.clone(),
            value: Operand::Int(2),
            order: AtomicOrdering::SeqCst,
        },
        loc(),
        "atomic_rmw_add",
    ).with_dest("old").with_ty(PrimType::I32));
    b.instrs.push(Instruction::new(
        InstructionKind::Cmpxchg {
            addr: addr.clone(),
            expected: Operand::Int(7),
            replacement: Operand::Int(9),
            order: AtomicOrdering::AcqRel,
        },
        loc(),
        "cmpxchg",
    ).with_dest("old2").with_extra_dests(["ok"]).with_ty(PrimType::I32));
    b.instrs.push(Instruction::new(
        InstructionKind::Fence { order: AtomicOrdering::SeqCst },
        loc(),
        "fence",
    ));
    b.instrs.push(Instruction::new(
        InstructionKind::Return { value: Some(Operand::reg("ok")) },
        loc(),
        "return",
    ));
    f.blocks.push(b);
    p.functions.push(f);
    let c = emit_c(&p, &COptions::default()).unwrap();
    assert!(c.contains("__atomic_load_n"), "{c}");
    assert!(c.contains("__ATOMIC_ACQUIRE"), "{c}");
    assert!(c.contains("__atomic_store_n"), "{c}");
    assert!(c.contains("__ATOMIC_RELEASE"), "{c}");
    assert!(c.contains("__atomic_fetch_add"), "{c}");
    assert!(c.contains("__atomic_compare_exchange_n"), "{c}");
    assert!(c.contains("__atomic_thread_fence"), "{c}");
}

#[test]
fn emits_fallible_call_try_and_return() {
    let mut p = Program::new();
    let mut msig = Signature::new("half");
    msig.return_ty = PrimType::F64;
    msig.return_fallible = true;
    let mut mf = Function::new(msig, loc());
    let mut mb = Block::new("L_ENTRY", loc());
    mb.instrs.push(Instruction::new(
        InstructionKind::Return {
            value: Some(Operand::Float(0.5)),
        },
        loc(),
        "return",
    ));
    mf.blocks.push(mb);
    p.functions.push(mf);

    let mut f = Function::new(Signature::new("main"), loc());
    let mut b = Block::new("L_ENTRY", loc());
    b.instrs.push(Instruction::new(
        InstructionKind::Call {
            target: "half".to_string(),
            args: vec![],
        },
        loc(),
        "call",
    ).with_dest("r"));
    b.instrs.push(Instruction::new(
        InstructionKind::Try {
            src: Operand::reg("r"),
        },
        loc(),
        "?",
    ).with_dest("v"));
    b.instrs.push(Instruction::new(
        InstructionKind::Return {
            value: Some(Operand::reg("v")),
        },
        loc(),
        "return",
    ));
    f.blocks.push(b);
    p.functions.push(f);
    let c = emit_c(&p, &COptions::default()).unwrap();
    assert!(c.contains("sa_ok_val("), "{c}");
    assert!(c.contains("sa_t = ("), "{c}");
    assert!(c.contains("double r1_v"), "{c}");
    assert!(!c.contains("not implemented by the C backend"), "{c}");
}

#[test]
fn emits_unknown_extern_as_trap_stub() {
    let mut p = Program::new();
    let mut sig = Signature::new("puts");
    sig.params.push(ParamSpec::new("s", PrimType::Ptr, CapPrefix::Borrow));
    p.externs.push(sig);
    let c = emit_c(&p, &COptions::default()).unwrap();
    assert!(c.contains("static void sa_ext0_puts"), "{c}");
    assert!(c.contains("sa_panic();"), "{c}");
}

#[test]
fn emits_staged_extern_shims() {
    let mut p = Program::new();
    let mut fd_open = Signature::new("fd_open");
    fd_open.return_ty = PrimType::I32;
    let mut memcpy = Signature::new("memcpy");
    memcpy.params.push(ParamSpec::new("dst", PrimType::Ptr, CapPrefix::Raw));
    memcpy.params.push(ParamSpec::new("src", PrimType::Ptr, CapPrefix::Borrow));
    memcpy.params.push(ParamSpec::new("n", PrimType::U64, CapPrefix::ByValue));
    memcpy.return_ty = PrimType::Ptr;
    p.externs.push(fd_open);
    p.externs.push(memcpy);
    let c = emit_c(&p, &COptions::default()).unwrap();
    assert!(c.contains("sa_ext_fd_next++"), "{c}");
    assert!(c.contains("memcpy((void *)a0, (const void *)a1"), "{c}");
    assert!(c.contains("sa_ext_buf_new"), "{c}");
}

#[test]
fn rejects_npm_bind() {
    let mut p = hello_program();
    p.npm_binds.push(NpmBindDecl {
        extern_name: "sa_print_bytes".to_string(),
        spec: "node:os".to_string(),
        target: "totalmem".to_string(),
        shape: NpmBindShape::Call,
        loc: loc(),
    });
    let errs = emit_c(&p, &COptions::default()).unwrap_err();
    assert!(errs.iter().any(|d| d.trap == Trap::UnsupportedInstruction));
}

#[test]
fn emits_vtable_const_and_call_indirect() {
    let mut p = hello_program();
    p.consts.push(ConstDecl::new(
        "VT",
        ConstValue::VTable(vec![ConstValue::Utf8("draw:@draw".to_string())]),
    ));
    let mut sig = Signature::new("draw");
    sig.params.push(ParamSpec::new("self", PrimType::Ptr, CapPrefix::Borrow));
    sig.return_ty = PrimType::I32;
    let mut f = Function::new(sig, loc());
    let mut b = Block::new("L_ENTRY", loc());
    b.instrs.push(Instruction::new(
        InstructionKind::Assign { src: Operand::Symbol("VT".to_string()) },
        loc(),
        "t",
    ).with_dest("t"));
    b.instrs.push(Instruction::new(
        InstructionKind::Load {
            addr: sa_ir::Address::new(Operand::reg("t"), 0),
        },
        loc(),
        "fp",
    ).with_dest("fp").with_ty(PrimType::Ptr));
    b.instrs.push(Instruction::new(
        InstructionKind::CallIndirect {
            target: Operand::reg("fp"),
            args: vec![CallArg::new(CapPrefix::Borrow, Operand::reg("t"))],
        },
        loc(),
        "call_indirect",
    ).with_dest("v"));
    b.instrs.push(Instruction::new(
        InstructionKind::Return { value: Some(Operand::reg("v")) },
        loc(),
        "return",
    ));
    f.blocks.push(b);
    p.functions.push(f);
    let c = emit_c(&p, &COptions::default()).unwrap();
    assert!(c.contains("static const uint64_t sa_c1_VT[]"));
    assert!(c.contains("(uint64_t)(uintptr_t)&sars_f1_draw"));
    assert!(c.contains("((int32_t (*)(uint8_t *))(uintptr_t)("));
}

#[test]
fn rejects_cross_target() {
    let mut opts = COptions::default();
    opts.target = Some("aarch64-linux-gnu".to_string());
    let errs = emit_c(&hello_program(), &opts).unwrap_err();
    assert!(errs.iter().any(|d| d.trap == Trap::UnsupportedInstruction));
}
