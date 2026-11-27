use crate::mask::{mask_name, Op, TrapKind, TRUTH_TABLE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapReport {
    pub trap: TrapKind,
    pub line: u32,
    pub register: String,
    pub expected_mask: u16,
    pub actual_mask: u16,
    pub message: String,
    pub hint: String,
}

impl TrapReport {
    pub fn trap_name(&self) -> &'static str {
        trap_name(self.trap)
    }

    pub fn expected_mask_name(&self) -> &'static str {
        mask_name(self.expected_mask)
    }

    pub fn actual_mask_name(&self) -> &'static str {
        mask_name(self.actual_mask)
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"trap\":\"{}\",\"trap_code\":null,\"line\":{},\"source_line\":{},\
\"source_text\":null,\"original_text\":null,\"register\":\"{}\",\"registers\":[],\
\"expected_mask\":{},\"actual_mask\":{},\"expected_mask_name\":\"{}\",\
\"actual_mask_name\":\"{}\",\"upstream_loc\":null,\"upstream_file\":null,\
\"upstream_line\":0,\"upstream_col\":0,\"function\":null,\"is_ffi_wrapper\":null,\
\"message\":\"{}\",\"hint\":\"{}\"}}",
            self.trap_name(),
            self.line,
            self.line,
            self.register,
            self.expected_mask,
            self.actual_mask,
            self.expected_mask_name(),
            self.actual_mask_name(),
            self.message,
            self.hint,
        )
    }
}

fn trap_name(trap: TrapKind) -> &'static str {
    match trap {
        TrapKind::ReadWriteConflict => "ReadWriteConflict",
        TrapKind::BorrowConflict => "BorrowConflict",
        TrapKind::DoubleMutableBorrow => "DoubleMutableBorrow",
        TrapKind::UseAfterMove => "UseAfterMove",
        TrapKind::MemoryLeak => "MemoryLeak",
        TrapKind::IllegalUnsafeContext => "IllegalUnsafeContext",
        TrapKind::FfiOwnershipViolation => "FfiOwnershipViolation",
        TrapKind::StackEscape => "StackEscape",
        TrapKind::TransferIntoBorrow => "TransferIntoBorrow",
        TrapKind::TransferIntoImmutable => "TransferIntoImmutable",
    }
}

fn expected_precondition(op: Op) -> u16 {
    TRUTH_TABLE
        .iter()
        .find(|row| row.op == op && row.legal)
        .map(|row| row.prev_mask)
        .expect("every op has a legal table row")
}

fn hint_for(trap: TrapKind) -> &'static str {
    match trap {
        TrapKind::ReadWriteConflict => "release the shared borrow before taking a mutable one",
        TrapKind::BorrowConflict => "release or move the active borrow before this operation",
        TrapKind::DoubleMutableBorrow => "only one mutable borrow may be live at a time",
        TrapKind::UseAfterMove => "do not reuse a value after it was moved or released",
        TrapKind::MemoryLeak => "insert an explicit release before function exit",
        TrapKind::IllegalUnsafeContext => "this operation requires an unsafe context",
        TrapKind::FfiOwnershipViolation => "an ffi-borrowed value cannot transfer ownership",
        TrapKind::StackEscape => "a stack value cannot escape its frame",
        TrapKind::TransferIntoBorrow => "a borrowed register cannot receive ownership",
        TrapKind::TransferIntoImmutable => "an immutable register cannot receive ownership",
    }
}

pub fn report_from_decision(register: &str, op: Op, actual_mask: u16, line: u32) -> Option<TrapReport> {
    let (legal, _, trap) = crate::capability::CapabilityEngine::decide(actual_mask, op)?;
    if legal {
        return None;
    }
    let trap = trap.unwrap_or(TrapKind::BorrowConflict);
    let expected_mask = expected_precondition(op);
    let message = format!(
        "{} on register '{}': expected {}, found {}",
        trap_name(trap),
        register,
        mask_name(expected_mask),
        mask_name(actual_mask),
    );
    Some(TrapReport {
        trap,
        line,
        register: register.to_string(),
        expected_mask,
        actual_mask,
        message,
        hint: hint_for(trap).to_string(),
    })
}
