//! Stage-1 runtime integration: route uncoupled capability ops through the
//! A1 truth table (`mask::lookup`). Call/FFI ops intentionally return no plan
//! and stay on the `FnState`/`RegState` bypass table.

use crate::mask::{self, Op, TrapKind};
use crate::state::FnState;
use sa_ir::{Instruction, InstructionKind, Operand, Trap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupPlan {
    pub reg: String,
    pub op: Op,
    pub prev_mask: u16,
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

pub fn plan(ins: &Instruction, state: &FnState) -> Option<LookupPlan> {
    let (reg, op) = match &ins.kind {
        InstructionKind::Alloc { .. } | InstructionKind::StackAlloc { .. } => {
            (ins.dest.as_deref()?, Op::Alloc)
        }
        InstructionKind::Move { src } => (reg_of(src)?, Op::Move),
        InstructionKind::Release { reg } => {
            let op = if state.borrow_origin.contains_key(reg) {
                Op::ReleaseBorrow
            } else {
                Op::ReleaseOwn
            };
            (reg.as_str(), op)
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
            (reg, op)
        }
        _ => return None,
    };
    Some(LookupPlan {
        reg: reg.to_string(),
        op,
        prev_mask: state.mask_of(reg),
    })
}

pub fn apply(
    state: &mut FnState,
    plan: &LookupPlan,
    bypass_errored: bool,
) -> Option<Stage1Decision> {
    let row = mask::lookup(plan.prev_mask, plan.op)?;
    state.set_mask(&plan.reg, row.new_mask);
    let confirmed_trap = if row.legal || !bypass_errored {
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
