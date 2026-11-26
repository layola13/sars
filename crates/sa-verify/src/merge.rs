use crate::mask::{bit, TrapKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskSnapshot {
    pub masks: Vec<u16>,
    pub origins: Vec<Option<usize>>,
    pub lock_refs: Vec<u16>,
    pub flags: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeTrap {
    ReadWriteConflict,
    UseAfterMove,
    BorrowConflict,
    Divergence,
}

impl MergeTrap {
    pub fn as_trap_kind(self) -> Option<TrapKind> {
        match self {
            MergeTrap::ReadWriteConflict => Some(TrapKind::ReadWriteConflict),
            MergeTrap::UseAfterMove => Some(TrapKind::UseAfterMove),
            MergeTrap::BorrowConflict => Some(TrapKind::BorrowConflict),
            MergeTrap::Divergence => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    pub register: usize,
    pub left: u16,
    pub right: u16,
    pub trap: MergeTrap,
}

pub fn join_masks(left: u16, right: u16) -> (u16, Option<MergeTrap>) {
    if left == right {
        return (left, None);
    }
    let left_locked_read = left & bit::LOCKED_READ != 0;
    let left_locked_mut = left & bit::LOCKED_MUT != 0;
    let right_locked_read = right & bit::LOCKED_READ != 0;
    let right_locked_mut = right & bit::LOCKED_MUT != 0;
    if (left_locked_read && right_locked_mut) || (left_locked_mut && right_locked_read) {
        return (left | right, Some(MergeTrap::ReadWriteConflict));
    }
    let left_consumed = left & bit::CONSUMED != 0;
    let right_consumed = right & bit::CONSUMED != 0;
    if left_consumed != right_consumed {
        return (left | right, Some(MergeTrap::UseAfterMove));
    }
    let borrow_bits = bit::LOCKED_READ | bit::LOCKED_MUT | bit::BORROW_VIEW;
    if left & borrow_bits != right & borrow_bits {
        return (left | right, Some(MergeTrap::BorrowConflict));
    }
    (left | right, Some(MergeTrap::Divergence))
}

pub fn merge_snapshots(left: &MaskSnapshot, right: &MaskSnapshot) -> (MaskSnapshot, Vec<MergeConflict>) {
    let len = left.masks.len().min(right.masks.len());
    let mut joined = MaskSnapshot {
        masks: Vec::with_capacity(len),
        origins: Vec::with_capacity(len),
        lock_refs: Vec::with_capacity(len),
        flags: Vec::with_capacity(len),
    };
    let mut conflicts = Vec::new();
    for index in 0..len {
        let (mask, trap) = join_masks(left.masks[index], right.masks[index]);
        if let Some(trap) = trap {
            conflicts.push(MergeConflict {
                register: index,
                left: left.masks[index],
                right: right.masks[index],
                trap,
            });
        }
        joined.masks.push(mask);
        joined.origins.push(if left.origins[index] == right.origins[index] {
            left.origins[index]
        } else {
            None
        });
        joined.lock_refs.push(left.lock_refs[index].max(right.lock_refs[index]));
        joined.flags.push(left.flags[index] | right.flags[index]);
    }
    (joined, conflicts)
}
