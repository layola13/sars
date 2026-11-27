use sa_verify::capability::CapabilityEngine;
use sa_verify::mask::{bit, mask_components, TrapKind};
use sa_verify::merge::{merge_snapshots, MaskSnapshot, MergeTrap};

fn snap(masks: Vec<u16>) -> MaskSnapshot {
    let len = masks.len();
    MaskSnapshot {
        masks,
        origins: vec![None; len],
        lock_refs: vec![0; len],
        flags: vec![0; len],
    }
}

#[test]
fn a1d_snapshot_round_trip_preserves_all_tables() {
    let mut engine = CapabilityEngine::new(3);
    engine.note_alloc(0).unwrap();
    engine.note_alloc(1).unwrap();
    engine.start_read_borrow(1, 2).unwrap();
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.masks[0], 0x01);
    assert_eq!(snapshot.masks[1], 0x02);
    assert_eq!(snapshot.masks[2], 0x11);
    assert_eq!(snapshot.origins[2], Some(1));
    assert_eq!(snapshot.lock_refs[1], 1);

    let mut restored = CapabilityEngine::new(3);
    restored.restore(&snapshot);
    assert_eq!(restored.mask(2), 0x11);
    assert_eq!(restored.origin(2), Some(1));
    assert_eq!(restored.lock_refs(1), 1);
    restored.release_borrow(2).unwrap();
    assert_eq!(restored.mask(1), 0x01);
}

#[test]
fn a1d_equal_snapshots_merge_without_conflict() {
    let a = snap(vec![0x01, 0x00, 0x08]);
    let (joined, conflicts) = merge_snapshots(&a, &a);
    assert!(conflicts.is_empty());
    assert_eq!(joined.masks, a.masks);
}

#[test]
fn a1d_read_write_conflict_is_trap_not_warning() {
    let a = snap(vec![0x02]);
    let b = snap(vec![0x04]);
    let (joined, conflicts) = merge_snapshots(&a, &b);
    assert_eq!(joined.masks, vec![0x06]);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].trap, MergeTrap::ReadWriteConflict);
    assert_eq!(conflicts[0].trap.as_trap_kind(), Some(TrapKind::ReadWriteConflict));
}

#[test]
fn a1d_consumed_vs_active_is_use_after_move_trap() {
    let a = snap(vec![bit::CONSUMED]);
    let b = snap(vec![bit::ACTIVE]);
    let (_, conflicts) = merge_snapshots(&a, &b);
    assert_eq!(conflicts[0].trap, MergeTrap::UseAfterMove);
    assert_eq!(conflicts[0].trap.as_trap_kind(), Some(TrapKind::UseAfterMove));
}

#[test]
fn a1d_borrow_divergence_is_borrow_conflict_trap() {
    let a = snap(vec![0x11]);
    let b = snap(vec![0x01]);
    let (_, conflicts) = merge_snapshots(&a, &b);
    assert_eq!(conflicts[0].trap, MergeTrap::BorrowConflict);
    assert_eq!(conflicts[0].trap.as_trap_kind(), Some(TrapKind::BorrowConflict));
}

#[test]
fn a1d_benign_join_keeps_all_derived_bits() {
    let a = snap(vec![0x101]);
    let b = snap(vec![0x281]);
    let (joined, conflicts) = merge_snapshots(&a, &b);
    assert_eq!(joined.masks, vec![0x381]);
    assert_eq!(conflicts[0].trap, MergeTrap::Divergence);
    assert_eq!(conflicts[0].trap.as_trap_kind(), None);
    assert_eq!(
        mask_components(joined.masks[0]),
        ["Active", "Fallible", "Immutable", "InteriorPtr"]
    );
}

#[test]
fn a1d_merge_keeps_max_lock_refs_and_equal_origins() {
    let a = MaskSnapshot {
        masks: vec![0x02, 0x11],
        origins: vec![None, Some(0)],
        lock_refs: vec![2, 1],
        flags: vec![0x01, 0x00],
    };
    let b = MaskSnapshot {
        masks: vec![0x02, 0x11],
        origins: vec![None, Some(9)],
        lock_refs: vec![1, 3],
        flags: vec![0x02, 0x00],
    };
    let (joined, conflicts) = merge_snapshots(&a, &b);
    assert!(conflicts.is_empty());
    assert_eq!(joined.lock_refs, vec![2, 3]);
    assert_eq!(joined.origins, vec![None, None]);
    assert_eq!(joined.flags, vec![0x03, 0x00]);
}
