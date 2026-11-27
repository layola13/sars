use sa_ir::{Instruction, InstructionKind, Operand, SourceLoc, Trap};
use sa_verify::mask::Op;
use sa_verify::runtime_mask::{apply, plan, Stage1Decision};
use sa_verify::state::{FnState, RegKind, RegState};

fn ins(kind: InstructionKind, dest: Option<&str>) -> Instruction {
    let mut instruction = Instruction::new(kind, SourceLoc::new("stage1.sa", 1, 1), "");
    instruction.dest = dest.map(str::to_string);
    instruction
}

fn reg(name: &str) -> Operand {
    Operand::Reg(name.to_string())
}

#[test]
fn stage1_uncoupled_ops_get_lookup_plans() {
    let state = FnState::default();
    let alloc = plan(
        &ins(InstructionKind::Alloc { size: reg("n") }, Some("a")),
        &state,
    )
    .expect("alloc is uncoupled");
    assert_eq!(alloc.op, Op::Alloc);
    assert_eq!(alloc.reg, "a");
    assert_eq!(alloc.prev_mask, 0x00);

    let move_plan = plan(&ins(InstructionKind::Move { src: reg("a") }, Some("b")), &state)
        .expect("move is uncoupled");
    assert_eq!(move_plan.op, Op::Move);
    assert_eq!(move_plan.reg, "a");

    let mut borrowed = FnState::default();
    borrowed.set("a", RegState::Borrowed { shared: 1, mutable: false });
    let borrow_again = plan(
        &ins(InstructionKind::Borrow { src: reg("a"), mutable: false }, Some("v")),
        &borrowed,
    )
    .expect("borrow is uncoupled");
    assert_eq!(borrow_again.op, Op::BorrowReadAgain);
    assert_eq!(borrow_again.prev_mask, 0x02);
}

#[test]
fn stage3_call_ops_stay_on_bypass_ffi_ops_get_plans() {
    let state = FnState::default();
    for kind in [
        InstructionKind::Call { target: "f".into(), args: vec![] },
        InstructionKind::CallIndirect { target: reg("fp"), args: vec![] },
        InstructionKind::Try { src: reg("a") },
    ] {
        assert!(plan(&ins(kind, Some("out")), &state).is_none());
    }
    for (kind, expected) in [
        (InstructionKind::RawCast { src: reg("a") }, Op::FfiRawCast),
        (InstructionKind::AssumeSafe { src: reg("a") }, Op::FfiAssumeSafe),
        (
            InstructionKind::AssumeBorrow { src: reg("a"), mutable: false },
            Op::FfiAssumeBorrow,
        ),
    ] {
        let plan = plan(&ins(kind, Some("out")), &state).expect("ffi op has a truth-table plan");
        assert_eq!(plan.op, expected);
        assert_eq!(plan.reg, "a");
        assert_eq!(plan.ffi_dest.as_deref(), Some("out"));
    }
}

#[test]
fn stage1_table_drives_mask_state_for_legal_rows() {
    let mut state = FnState::default();
    let alloc = plan(
        &ins(InstructionKind::Alloc { size: reg("n") }, Some("a")),
        &state,
    )
    .unwrap();
    let decision = apply(&mut state, &alloc, false).expect("0x00+Alloc is on the table");
    assert_eq!(
        decision,
        Stage1Decision { legal: true, new_mask: 0x01, confirmed_trap: None }
    );
    assert_eq!(state.mask_of("a"), 0x01);

    let borrow = plan(
        &ins(InstructionKind::Borrow { src: reg("a"), mutable: false }, Some("v")),
        &state,
    )
    .unwrap();
    assert_eq!(borrow.op, Op::BorrowRead);
    let decision = apply(&mut state, &borrow, false).unwrap();
    assert_eq!(decision.new_mask, 0x02);
    assert_eq!(state.mask_of("a"), 0x02);
}

#[test]
fn stage3_illegal_rows_are_authoritative_except_alloc() {
    let mut state = FnState::default();
    state.set("a", RegState::Borrowed { shared: 1, mutable: false });
    let plan = plan(
        &ins(InstructionKind::Borrow { src: reg("a"), mutable: true }, Some("v")),
        &state,
    )
    .unwrap();
    assert_eq!(plan.op, Op::BorrowMut);

    let confirmed = apply(&mut state, &plan, true).unwrap();
    assert!(!confirmed.legal);
    assert_eq!(confirmed.confirmed_trap, None);

    let authoritative = apply(&mut state, &plan, false).unwrap();
    assert_eq!(authoritative.confirmed_trap, Some(Trap::ReadWriteConflict));
}

#[test]
fn stage1_unlisted_pair_defers_to_bypass() {
    let mut state = FnState::default();
    state.set("a", RegState::Borrowed { shared: 1, mutable: false });
    let plan = plan(
        &ins(InstructionKind::Alloc { size: reg("n") }, Some("a")),
        &state,
    )
    .unwrap();
    assert_eq!(plan.op, Op::Alloc);
    assert_eq!(plan.prev_mask, 0x02);
    assert!(apply(&mut state, &plan, true).is_none());
    assert_eq!(state.mask_of("a"), 0x02);
}

#[test]
fn stage3_alloc_on_consumed_defers_to_live_notealloc_semantics() {
    let mut state = FnState::default();
    state.set("a", RegState::Moved);
    state.set_mask("a", 0x08);
    let plan = plan(
        &ins(InstructionKind::Alloc { size: reg("n") }, Some("a")),
        &state,
    )
    .unwrap();
    let decision = apply(&mut state, &plan, false).unwrap();
    assert!(!decision.legal);
    assert_eq!(decision.confirmed_trap, None);
}

#[test]
fn stage1_release_borrow_view_uses_release_borrow_row() {
    let mut state = FnState::default();
    state.set("a", RegState::Owned { kind: RegKind::Alloc });
    state.set("v", RegState::Borrowed { shared: 1, mutable: false });
    state.borrow_origin.insert("v".into(), "a".into());
    let plan = plan(&ins(InstructionKind::Release { reg: "v".into() }, None), &state).unwrap();
    assert_eq!(plan.op, Op::ReleaseBorrow);
    assert_eq!(plan.prev_mask, 0x11);
    let decision = apply(&mut state, &plan, false).unwrap();
    assert_eq!(decision.new_mask, 0x01);
    assert_eq!(state.mask_of("v"), 0x01);
}

#[test]
fn stage1_mask_state_joins_across_merge() {
    let mut left = FnState::default();
    left.set("a", RegState::Owned { kind: RegKind::Alloc });
    left.set_mask("a", 0x01);
    let mut right = FnState::default();
    right.set("a", RegState::Owned { kind: RegKind::Alloc });
    right.set_mask("a", 0x100);
    let (merged, conflicts) = left.merge(&right);
    assert!(conflicts.is_empty());
    assert_eq!(merged.mask_of("a"), 0x101);
}
