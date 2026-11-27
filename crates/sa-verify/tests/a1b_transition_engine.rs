use sa_verify::capability::{CapabilityEngine, EngineError};
use sa_verify::mask::{bit, lookup, Op, TrapKind, TRUTH_TABLE};

#[test]
fn a1b_decide_matches_every_sci_truth_table_row() {
    for (index, row) in TRUTH_TABLE.iter().enumerate() {
        let got = CapabilityEngine::decide(row.prev_mask, row.op);
        assert_eq!(
            got,
            Some((row.legal, row.new_mask, row.trap)),
            "truth-table row {index} ({:#x}, {:?}) did not drive the engine",
            row.prev_mask,
            row.op,
        );
        assert_eq!(lookup(row.prev_mask, row.op).and_then(|r| r.trap), row.trap);
    }
}

#[test]
fn a1b_decide_has_no_if_else_fallback_for_unlisted_pairs() {
    assert_eq!(CapabilityEngine::decide(0x02, Op::Alloc), None);
    assert_eq!(CapabilityEngine::decide(0x08, Op::BorrowMut), None);
    assert_eq!(CapabilityEngine::decide(0x01, Op::FfiRawCast), None);
}

#[test]
fn a1b_alloc_borrow_move_release_parity() {
    let mut engine = CapabilityEngine::new(4);
    engine.note_alloc(0).unwrap();
    assert_eq!(engine.mask(0), 0x01);
    engine.note_move(0).unwrap();
    assert_eq!(engine.mask(0), 0x08);

    engine.note_alloc(1).unwrap();
    engine.start_read_borrow(1, 2).unwrap();
    assert_eq!(engine.mask(1), 0x02);
    assert_eq!(engine.mask(2), 0x11);
    assert_eq!(engine.origin(2), Some(1));
    assert_eq!(engine.lock_refs(1), 1);
    engine.release_borrow(2).unwrap();
    assert_eq!(engine.mask(1), 0x01);
    assert_eq!(engine.mask(2), 0x00);
}

#[test]
fn a1b_second_exclusive_borrow_is_double_mutable_borrow() {
    let mut engine = CapabilityEngine::new(4);
    engine.note_alloc(0).unwrap();
    engine.start_mut_borrow(0, 1).unwrap();
    assert_eq!(engine.mask(0), 0x04);
    assert_eq!(engine.mask(1), 0x11);
    assert_eq!(
        engine.start_mut_borrow(0, 2),
        Err(EngineError::DoubleMutableBorrow)
    );
}

#[test]
fn a1b_shared_then_mutable_borrow_is_read_write_conflict() {
    let mut engine = CapabilityEngine::new(4);
    engine.note_alloc(0).unwrap();
    engine.start_read_borrow(0, 1).unwrap();
    assert_eq!(engine.mask(0), 0x02);
    assert_eq!(
        engine.start_mut_borrow(0, 2),
        Err(EngineError::ReadWriteConflict)
    );
    let decided = CapabilityEngine::decide(0x02, Op::BorrowMut).unwrap();
    assert_eq!(decided.2, Some(TrapKind::ReadWriteConflict));
}

#[test]
fn a1b_move_after_consume_is_use_after_move() {
    let mut engine = CapabilityEngine::new(2);
    engine.note_alloc(0).unwrap();
    engine.note_move(0).unwrap();
    assert_eq!(engine.mask(0), bit::CONSUMED);
    assert_eq!(engine.note_move(0), Err(EngineError::UseAfterMove));
    let decided = CapabilityEngine::decide(0x08, Op::Alloc).unwrap();
    assert_eq!(decided.2, Some(TrapKind::UseAfterMove));
}

#[test]
fn a1b_move_out_of_borrow_is_borrow_conflict() {
    let mut engine = CapabilityEngine::new(3);
    engine.note_alloc(0).unwrap();
    engine.start_read_borrow(0, 1).unwrap();
    assert_eq!(engine.note_move(0), Err(EngineError::BorrowConflict));
    engine.note_release_own(0).unwrap_err();

    let mut other = CapabilityEngine::new(2);
    other.note_alloc(0).unwrap();
    other.start_mut_borrow(0, 1).unwrap();
    assert_eq!(
        CapabilityEngine::decide(0x04, Op::Move).unwrap().2,
        Some(TrapKind::BorrowConflict)
    );
    assert_eq!(other.note_move(0), Err(EngineError::BorrowConflict));
}

#[test]
fn a1b_ffi_borrow_view_carries_four_bits_without_simplification() {
    let mut engine = CapabilityEngine::new(3);
    engine.bind_raw(0).unwrap();
    engine.start_read_borrow(0, 1).unwrap();
    assert_eq!(engine.mask(1), 0x33);
    assert_eq!(engine.mask(1) & (bit::ACTIVE | bit::LOCKED_READ | bit::BORROW_VIEW | bit::FFI_BORROW), 0x33);
    assert_eq!(engine.note_move(1), Err(EngineError::FfiOwnershipViolation));
    engine.release_borrow(1).unwrap();
    assert_eq!(engine.mask(1), 0x00);
}
