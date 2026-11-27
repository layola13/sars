//! A1 convergence runtime integration: route alloc/borrow/move/release and
//! FFI reinterpretation ops through the truth table (`mask::lookup`). Call,
//! CallIndirect and Try still use the imperative capability rules; their
//! fallible/signature state is broader than the 20-row transition table.

use crate::mask::{self, Op, TrapKind};
use crate::state::RegStateBackend;
use sa_ir::{Instruction, InstructionKind, Operand, Trap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupPlan {
    pub reg: String,
    pub op: Op,
    pub prev_mask: u16,
    /// Registers that receive `new_mask` when the transition is a FFI
    /// reinterpretation (`raw_cast` / `assume_safe` / `assume_borrow`). Borrow
    /// views are intentionally excluded: their mask is derived from the
    /// source plus the borrow-view bit by `add_borrow`.
    pub ffi_dest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stage1Decision {
    pub legal: bool,
    pub new_mask: u16,
    pub confirmed_trap: Option<Trap>,
}

pub fn trap_of(trap: TrapKind) -> Trap {
    match trap {
        TrapKind::ReadWriteConflict => Trap::ReadWriteConflict,
        TrapKind::BorrowConflict => Trap::BorrowConflict,
        TrapKind::DoubleMutableBorrow => Trap::DoubleMutableBorrow,
        TrapKind::UseAfterMove => Trap::UseAfterMove,
        TrapKind::MemoryLeak => Trap::MemoryLeak,
        TrapKind::IllegalUnsafeContext => Trap::IllegalUnsafeContext,
        TrapKind::FfiOwnershipViolation => Trap::FfiOwnershipViolation,
        TrapKind::StackEscape => Trap::StackEscape,
        TrapKind::TransferIntoBorrow => Trap::TransferIntoBorrow,
        TrapKind::TransferIntoImmutable => Trap::TransferIntoImmutable,
    }
}

pub fn plan<S: RegStateBackend>(ins: &Instruction, state: &S) -> Option<LookupPlan> {
    let (reg, op, ffi_dest) = match &ins.kind {
        InstructionKind::Alloc { .. } | InstructionKind::StackAlloc { .. } => {
            (ins.dest.as_deref()?, Op::Alloc, None)
        }
        InstructionKind::Move { src } => (reg_of(src)?, Op::Move, None),
        InstructionKind::Release { reg } => {
            let op = if state.borrow_origin_contains(reg) {
                Op::ReleaseBorrow
            } else {
                Op::ReleaseOwn
            };
            (reg.as_str(), op, None)
        }
        InstructionKind::Borrow { src, mutable } => {
            let reg = reg_of(src)?;
            let again = matches!(
                state.state_of(reg),
                crate::state::RegState::Borrowed { shared: n, mutable: false } if n > 0
            );
            let op = if *mutable {
                Op::BorrowMut
            } else if again {
                Op::BorrowReadAgain
            } else {
                Op::BorrowRead
            };
            (reg, op, None)
        }
        InstructionKind::RawCast { src } => (reg_of(src)?, Op::FfiRawCast, ins.dest.clone()),
        InstructionKind::AssumeSafe { src } => (reg_of(src)?, Op::FfiAssumeSafe, ins.dest.clone()),
        InstructionKind::AssumeBorrow { src, .. } => {
            (reg_of(src)?, Op::FfiAssumeBorrow, ins.dest.clone())
        }
        _ => return None,
    };
    Some(LookupPlan {
        reg: reg.to_string(),
        op,
        prev_mask: state.mask_of(reg),
        ffi_dest,
    })
}

pub fn apply<S: RegStateBackend>(
    state: &mut S,
    plan: &LookupPlan,
    bypass_errored: bool,
) -> Option<Stage1Decision> {
    let row = mask::lookup(plan.prev_mask, plan.op)?;
    state.set_mask(&plan.reg, row.new_mask);
    if let Some(dest) = &plan.ffi_dest {
        state.set_mask(dest, row.new_mask);
    }
    // The truth table is the primary legality authority: an illegal row is
    // reported even when the imperative bypass rules accepted the operation.
    // When the bypass already reported a diagnostic, keep the legacy message
    // and avoid a duplicate; the mask transition still lands.
    // `Alloc` is confirmation-only on purpose. The canonical 0x08 row in the
    // 20-row table is a teaching invariant, while sci's live
    // `CapabilityTable.noteAlloc` explicitly restarts a consumed slot as
    // Active. Keeping Alloc on the bypass path preserves that live semantics;
    // every other planned op is authoritative.
    let confirmation_only = matches!(plan.op, Op::Alloc);
    let confirmed_trap = if row.legal || bypass_errored || confirmation_only {
        None
    } else {
        Some(row.trap.map(trap_of).unwrap_or(Trap::CapabilityMismatch))
    };
    Some(Stage1Decision {
        legal: row.legal,
        new_mask: row.new_mask,
        confirmed_trap,
    })
}

fn reg_of(op: &Operand) -> Option<&str> {
    match op {
        Operand::Reg(name) => Some(name.as_str()),
        _ => None,
    }
}
