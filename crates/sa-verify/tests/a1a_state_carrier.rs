use sa_verify::mask::{
    bit, mask_components, mask_name, CapabilityMask, Op, Transition, TrapKind, TRUTH_TABLE,
};

#[test]
fn a1a_twelve_bit_constants_match_sci() {
    assert_eq!(bit::UNINITIALIZED, 0x0000);
    assert_eq!(bit::ACTIVE, 0x0001);
    assert_eq!(bit::LOCKED_READ, 0x0002);
    assert_eq!(bit::LOCKED_MUT, 0x0004);
    assert_eq!(bit::CONSUMED, 0x0008);
    assert_eq!(bit::BORROW_VIEW, 0x0010);
    assert_eq!(bit::FFI_BORROW, 0x0020);
    assert_eq!(bit::UNTRACKED, 0x0040);
    assert_eq!(bit::FALLIBLE, 0x0080);
    assert_eq!(bit::IMMUTABLE, 0x0100);
    assert_eq!(bit::INTERIOR_PTR, 0x0200);
    assert_eq!(bit::OWNS_CHILDREN, 0x0400);
}

#[test]
fn a1a_composite_masks_are_representable() {
    let active_immutable = CapabilityMask::new(0x101);
    let ffi_borrow_view = CapabilityMask::new(0x33);
    assert_eq!(active_immutable.bits(), 0x101);
    assert_eq!(ffi_borrow_view.bits(), 0x33);
    assert!(active_immutable.contains(bit::ACTIVE));
    assert!(active_immutable.contains(bit::IMMUTABLE));
    assert!(ffi_borrow_view.contains(bit::BORROW_VIEW));
    assert!(ffi_borrow_view.contains(bit::FFI_BORROW));
    assert_eq!(mask_components(active_immutable.bits()), ["Active", "Immutable"]);
    assert_eq!(
        mask_components(ffi_borrow_view.bits()),
        ["Active", "Locked_Read", "BorrowView", "FfiBorrow"]
    );
    assert_eq!(mask_name(active_immutable.bits()), "Composite");
    assert_eq!(mask_name(ffi_borrow_view.bits()), "Composite");
}

#[test]
fn a1a_mask_name_matches_sci_canonical_states() {
    assert_eq!(mask_name(0x0000), "Uninitialized");
    assert_eq!(mask_name(0x0001), "Active");
    assert_eq!(mask_name(0x0002), "Locked_Read");
    assert_eq!(mask_name(0x0004), "Locked_Mut");
    assert_eq!(mask_name(0x0008), "Consumed");
    assert_eq!(mask_name(0x0010), "BorrowView");
    assert_eq!(mask_name(0x0020), "FfiBorrow");
    assert_eq!(mask_name(0x0040), "Untracked");
    assert_eq!(mask_name(0x0080), "Fallible");
    assert_eq!(mask_name(0x0100), "Immutable");
    assert_eq!(mask_name(0x0200), "InteriorPtr");
    assert_eq!(mask_name(0x0400), "OwnsChildren");
}

#[test]
fn a1a_truth_table_matches_sci_20_entries() {
    let expected: [(u16, Op, bool, u16, Option<TrapKind>); 20] = [
        (0x00, Op::Alloc, true, 0x01, None),
        (0x01, Op::BorrowRead, true, 0x02, None),
        (0x01, Op::BorrowMut, true, 0x04, None),
        (0x02, Op::BorrowReadAgain, true, 0x02, None),
        (0x02, Op::BorrowMut, false, 0x00, Some(TrapKind::ReadWriteConflict)),
        (0x04, Op::BorrowRead, false, 0x00, Some(TrapKind::BorrowConflict)),
        (0x04, Op::BorrowMut, false, 0x00, Some(TrapKind::DoubleMutableBorrow)),
        (0x04, Op::Move, false, 0x00, Some(TrapKind::BorrowConflict)),
        (0x01, Op::Move, true, 0x08, None),
        (0x08, Op::Alloc, false, 0x00, Some(TrapKind::UseAfterMove)),
        (0x11, Op::ReleaseBorrow, true, 0x01, None),
        (0x01, Op::ReleaseOwn, true, 0x08, None),
        (0x40, Op::FfiRawCast, true, 0x40, None),
        (0x40, Op::FfiAssumeSafe, true, 0x01, None),
        (0x40, Op::FfiAssumeBorrow, true, 0x33, None),
        (0x100, Op::Alloc, true, 0x100, None),
        (0x200, Op::Move, true, 0x200, None),
        (0x01, Op::TransferOwn, true, 0x08, None),
        (0x10, Op::TransferOwn, false, 0x00, Some(TrapKind::TransferIntoBorrow)),
        (0x101, Op::TransferOwn, false, 0x00, Some(TrapKind::TransferIntoImmutable)),
    ];
    assert_eq!(TRUTH_TABLE.len(), 20);
    for (index, (row, want)) in TRUTH_TABLE.iter().zip(expected.iter()).enumerate() {
        let got = Transition {
            prev_mask: row.prev_mask,
            op: row.op,
            legal: row.legal,
            new_mask: row.new_mask,
            trap: row.trap,
        };
        let (prev_mask, op, legal, new_mask, trap) = *want;
        let want = Transition { prev_mask, op, legal, new_mask, trap };
        assert_eq!(got, want, "truth table row {index} differs from sci");
    }
}

#[test]
fn a1a_composite_states_anchor_the_truth_table() {
    let ffi_assume_borrow = TRUTH_TABLE
        .iter()
        .find(|row| row.op == Op::FfiAssumeBorrow)
        .expect("ffi_assume_borrow row");
    assert_eq!(ffi_assume_borrow.new_mask, 0x33);
    assert_eq!(
        mask_components(ffi_assume_borrow.new_mask),
        ["Active", "Locked_Read", "BorrowView", "FfiBorrow"]
    );

    let immutable_transfer = TRUTH_TABLE
        .iter()
        .find(|row| row.prev_mask == 0x101 && row.op == Op::TransferOwn)
        .expect("transfer_own into active|immutable row");
    assert!(!immutable_transfer.legal);
    assert_eq!(immutable_transfer.trap, Some(TrapKind::TransferIntoImmutable));
    assert_eq!(mask_components(immutable_transfer.prev_mask), ["Active", "Immutable"]);
}
