//! B3: named @const symbols are immutable — move/release/return-by-move are
//! ConstMutation (sci seedConstSymbols + writeCheck/constTrap).
use sa_ir::{
    Block, ConstDecl, ConstValue, Diagnostic, Function, Instruction, InstructionKind, Operand,
    PrimType, Program, Signature, SourceLoc, Trap,
};
use sa_verify::{verify, VerifyOptions};

fn loc(line: u32) -> SourceLoc {
    SourceLoc::new("test.sa", line, 1)
}

fn ins(kind: InstructionKind, line: u32) -> Instruction {
    Instruction::new(kind, loc(line), "")
}

fn check(instrs: Vec<Instruction>) -> Vec<Diagnostic> {
    let mut b = Block::new("L_entry", loc(1));
    b.instrs = instrs;
    let mut f = Function::new(Signature::new("main"), loc(1));
    f.blocks = vec![b];
    let mut p = Program::new();
    p.consts.push(ConstDecl::new("K", ConstValue::Int(42)));
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

fn has_const_mutation(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.trap == Trap::ConstMutation)
}

#[test]
fn b3_move_of_const_is_const_mutation() {
    let diags = check(vec![
        ins(InstructionKind::Move { src: Operand::Reg("K".into()) }, 2).with_dest("x"),
        ins(InstructionKind::Return { value: None }, 3),
    ]);
    assert!(has_const_mutation(&diags), "{diags:?}");
}

#[test]
fn b3_release_of_const_is_const_mutation() {
    let diags = check(vec![
        ins(InstructionKind::Release { reg: "K".into() }, 2),
        ins(InstructionKind::Return { value: None }, 3),
    ]);
    assert!(has_const_mutation(&diags), "{diags:?}");
}

#[test]
fn b3_return_const_by_move_is_const_mutation() {
    let diags = check(vec![
        ins(InstructionKind::Return { value: Some(Operand::Reg("K".into())) }, 2),
    ]);
    assert!(has_const_mutation(&diags), "{diags:?}");
}

#[test]
fn b3_const_read_and_borrow_stay_legal() {
    // Copying/reading a const, and a shared borrow, must remain allowed.
    let mut sig = Signature::new("main");
    sig.return_ty = PrimType::Void;
    let mut b = Block::new("L_entry", loc(1));
    let mut load = ins(
        InstructionKind::Load { addr: sa_ir::Address::new(Operand::Reg("K".into()), 0) },
        2,
    );
    load.dest = Some("v".into());
    b.instrs = vec![load, ins(InstructionKind::Return { value: None }, 3)];
    let mut f = Function::new(sig, loc(1));
    f.blocks = vec![b];
    let mut p = Program::new();
    p.consts.push(ConstDecl::new("K", ConstValue::Int(42)));
    p.functions.push(f);
    let opts = VerifyOptions::default();
    assert!(verify(&p, &opts).is_ok(), "reading a const must not be a mutation");
}
