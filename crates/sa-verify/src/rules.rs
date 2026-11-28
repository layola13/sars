use crate::mask::bit;
use crate::state::{reg_flag, RegKind, RegState, RegStateBackend};
use crate::{DiagSink, VerifyOptions};
use sa_ir::{
    Capability, CapPrefix, Diagnostic, FunctionKind, Instruction, InstructionKind, Operand,
    PrimType, Program, Signature, SourceLoc, Trap,
};
use std::collections::HashMap;

pub struct RuleCtx<'a> {
    pub program: &'a Program,
    pub opts: &'a VerifyOptions,
    pub sink: &'a mut DiagSink,
    pub func_name: &'a str,
    pub signature: &'a Signature,
    pub atomics: &'a mut HashMap<String, u8>,
}

impl<'a> RuleCtx<'a> {
    pub(crate) fn report(
        &mut self,
        trap: Trap,
        message: impl Into<String>,
        loc: &SourceLoc,
        reg: Option<&str>,
        hint: Option<String>,
        warning: bool,
    ) {
        let mut diagnostic =
            Diagnostic::new(trap, message, loc.clone()).in_function(self.func_name);
        if let Some(name) = reg {
            diagnostic = diagnostic.for_register(name);
        }
        if let Some(text) = hint {
            diagnostic = diagnostic.with_hint(text);
        }
        if warning {
            self.sink.push_warning(diagnostic);
        } else {
            self.sink.push_error(diagnostic);
        }
    }
}

fn check_redefine<S: RegStateBackend>(ctx: &mut RuleCtx, state: &S, reg: &str, loc: &SourceLoc) {
    if state.escaped_contains(reg) {
        return;
    }
    match state.state_of(reg) {
        RegState::Owned { kind: RegKind::Scalar } => return,
        RegState::Owned { .. } => ctx.report(
            Trap::RegisterRedefinition,
            format!("register '{reg}' is already live"),
            loc,
            Some(reg),
            Some("release or move the old value before defining it again".into()),
            false,
        ),
        RegState::Borrowed { mutable: true, .. } => ctx.report(
            Trap::BorrowConflict,
            format!("register '{reg}' is exclusively borrowed and cannot be redefined"),
            loc,
            Some(reg),
            None,
            false,
        ),
        RegState::Borrowed { .. } => ctx.report(
            Trap::BorrowConflict,
            format!("register '{reg}' has active borrows and cannot be redefined"),
            loc,
            Some(reg),
            None,
            false,
        ),
        _ => {}
    }
}

fn define<S: RegStateBackend>(ctx: &mut RuleCtx, ins: &Instruction, state: &mut S, new_state: RegState, loc: &SourceLoc) {
    for dest in ins.all_dests() {
        let self_update = matches!(state.state_of(dest), RegState::Owned { .. })
            && ins.reads().iter().any(|op| matches!(op, Operand::Reg(name) if name == dest));
        if !self_update {
            check_redefine(ctx, state, dest, loc);
        }
        state.set(dest, new_state.clone());
        // sci assignValueCtx clears BRANCH_COND|EPHEMERAL on every
        // (re)definition; arm-specific flag writes in apply_reg_flags run
        // after this, mirroring sci arms setting flags after assignValue.
        state.flag_clear(dest, reg_flag::BRANCH_COND | reg_flag::EPHEMERAL);
    }
}

fn is_constant(ctx: &RuleCtx, op: &Operand) -> bool {
    match op {
        Operand::Reg(name) => ctx.program.consts.iter().any(|c| c.name == *name),
        _ => op.is_const(),
    }
}

/// A named `@const` symbol (as opposed to a numeric literal). sci seeds these
/// as `active | immutable` (`const_trap.zig:44-55`) and rejects move/release/
/// write/return by move; sars tracks them via `program.consts`.
fn is_named_const<'a>(ctx: &RuleCtx, op: &'a Operand) -> Option<&'a str> {
    match op {
        Operand::Reg(name) if ctx.program.consts.iter().any(|c| c.name == *name) => Some(name),
        _ => None,
    }
}

fn const_mutation(
    ctx: &mut RuleCtx,
    op: &Operand,
    action: &str,
    loc: &SourceLoc,
) -> bool {
    if let Some(name) = is_named_const(ctx, op) {
        ctx.report(
            Trap::ConstMutation,
            format!("immutable constant '{name}' cannot be {action}"),
            loc,
            Some(name),
            None,
            false,
        );
        return true;
    }
    false
}

fn read_operand<S: RegStateBackend>(ctx: &mut RuleCtx, op: &Operand, state: &S, loc: &SourceLoc) {
    if is_constant(ctx, op) {
        return;
    }
    let name = match op {
        Operand::Reg(name) => name,
        _ => return,
    };
    match state.state_of(name) {
        RegState::Undefined => ctx.report(
            Trap::UnknownRegister,
            format!("register '{name}' is not declared in the current scope"),
            loc,
            Some(name),
            None,
            false,
        ),
        RegState::Moved | RegState::Released => ctx.report(
            Trap::UseAfterMove,
            format!("moved value '{name}' is no longer usable"),
            loc,
            Some(name),
            Some(format!("do not read '{name}' after it was moved or released")),
            false,
        ),
        RegState::Borrowed { mutable: true, .. } => {
            if !state.borrow_origin_contains(name) {
                ctx.report(
                    Trap::BorrowConflict,
                    format!("register '{name}' is exclusively borrowed and cannot be read"),
                    loc,
                    Some(name),
                    None,
                    false,
                );
            }
        }
        _ => {}
    }
}

fn consume<S: RegStateBackend>(ctx: &mut RuleCtx, state: &mut S, op: &Operand, loc: &SourceLoc) {
    // sci writeCheck/consumeValue: const data cannot be moved (it has no
    // ownership to transfer). Literals still pass through as no-ops.
    if const_mutation(ctx, op, "moved", loc) {
        return;
    }
    if is_constant(ctx, op) {
        return;
    }
    let name = match op {
        Operand::Reg(name) => name,
        _ => return,
    };
    match state.state_of(name) {
        RegState::Undefined => ctx.report(
            Trap::UnknownRegister,
            format!("register '{name}' is not declared in the current scope"),
            loc,
            Some(name),
            None,
            false,
        ),
        RegState::Moved | RegState::Released => ctx.report(
            Trap::UseAfterMove,
            format!("moved value '{name}' cannot be consumed twice"),
            loc,
            Some(name),
            None,
            false,
        ),
        RegState::Borrowed { mutable: true, .. } => ctx.report(
            Trap::BorrowConflict,
            format!("cannot move out of exclusively borrowed register '{name}'"),
            loc,
            Some(name),
            None,
            false,
        ),
        RegState::Borrowed { shared, .. } if shared > 0 => ctx.report(
            Trap::BorrowConflict,
            format!("cannot move out of borrowed register '{name}'"),
            loc,
            Some(name),
            None,
            false,
        ),
        _ => state.set(name, RegState::Moved),
    }
}

fn is_scalar_operand<S: RegStateBackend>(_ctx: &RuleCtx, state: &S, op: &Operand) -> bool {
    match op {
        Operand::Reg(name) => matches!(
            state.state_of(name),
            RegState::Owned { kind: RegKind::Scalar }
        ),
        _ => !matches!(op, Operand::Symbol(_) | Operand::Label(_)),
    }
}

fn consume_assign<S: RegStateBackend>(ctx: &mut RuleCtx, state: &mut S, op: &Operand, loc: &SourceLoc) {
    if let Operand::Reg(name) = op {
        if matches!(state.state_of(name), RegState::Owned { kind: RegKind::Param }) {
            read_operand(ctx, op, state, loc);
            return;
        }
    }
    consume(ctx, state, op, loc);
}

fn add_borrow<S: RegStateBackend>(
    ctx: &mut RuleCtx,
    state: &mut S,
    op: &Operand,
    mutable: bool,
    view: Option<&str>,
    loc: &SourceLoc,
) {
    if is_constant(ctx, op) && !mutable {
        return;
    }
    let name = match op {
        Operand::Reg(name) => name,
        _ => {
            ctx.report(
                Trap::UnknownRegister,
                "borrow source must be a register",
                loc,
                None,
                None,
                false,
            );
            return;
        }
    };
    if mutable
        && ctx.opts.check_capabilities
        && ctx.program.consts.iter().any(|constant| constant.name == name.as_str())
    {
        ctx.report(
            Trap::CapabilityMismatch,
            format!("cannot mutably borrow constant '{name}'"),
            loc,
            Some(name),
            None,
            false,
        );
    }
    match state.state_of(name) {
        RegState::Undefined => ctx.report(
            Trap::UnknownRegister,
            format!("register '{name}' is not declared in the current scope"),
            loc,
            Some(name),
            None,
            false,
        ),
        RegState::Moved | RegState::Released => ctx.report(
            Trap::UseAfterMove,
            format!("moved value '{name}' is no longer usable"),
            loc,
            Some(name),
            None,
            false,
        ),
        RegState::Borrowed { shared, mutable: current } => {
            if mutable {
                if current {
                    ctx.report(
                        Trap::DoubleMutableBorrow,
                        format!("cannot borrow '{name}' mutably more than once"),
                        loc,
                        Some(name),
                        None,
                        false,
                    );
                } else if shared > 0 {
                    ctx.report(
                        Trap::BorrowConflict,
                        format!("cannot borrow '{name}' mutably while shared borrows are active"),
                        loc,
                        Some(name),
                        None,
                        false,
                    );
                } else {
                    state.set(name, RegState::Borrowed { shared: 0, mutable: true });
                }
            } else if current {
                ctx.report(
                    Trap::BorrowConflict,
                    format!("cannot borrow '{name}' while a mutable borrow is active"),
                    loc,
                    Some(name),
                    None,
                    false,
                );
            } else {
                state.set(name, RegState::Borrowed { shared: shared.saturating_add(1), mutable: false });
            }
        }
        owned @ RegState::Owned { .. } => {
            state.origin_base_entry_or_insert(name, owned);
            state.set(
                name,
                RegState::Borrowed { shared: if mutable { 0 } else { 1 }, mutable },
            );
        }
        RegState::Conflict => {}
    }
    if let Some(dest) = view {
        state.set(
            dest,
            RegState::Borrowed { shared: 1, mutable },
        );
        state.borrow_origin_insert(dest, name);
    }
}

fn release<S: RegStateBackend>(ctx: &mut RuleCtx, state: &mut S, reg: &str, loc: &SourceLoc) {
    // sci assign_branch.zig:85-86: releasing const data is a const mutation.
    if const_mutation(ctx, &Operand::Reg(reg.to_string()), "released", loc) {
        return;
    }
    if matches!(state.state_of(reg), RegState::Owned { kind: RegKind::Scalar }) {
        return;
    }
    match state.state_of(reg) {
        RegState::Undefined => ctx.report(
            Trap::UnknownRegister,
            format!("register '{reg}' is not declared in the current scope"),
            loc,
            Some(reg),
            None,
            false,
        ),
        RegState::Moved | RegState::Released => ctx.report(
            Trap::UseAfterMove,
            format!("register '{reg}' was already consumed"),
            loc,
            Some(reg),
            None,
            false,
        ),
        RegState::Owned { .. } => {
            state.origin_base_remove(reg);
            state.set(reg, RegState::Released);
        }
        RegState::Borrowed { shared, mutable } => {
            if let Some(origin) = state.borrow_origin_remove(reg) {
                let mut restore = false;
                if let RegState::Borrowed { shared: live, mutable: locked } = state.state_of(&origin) {
                    let next_shared = if mutable { live } else { live.saturating_sub(1) };
                    let next_mutable = if mutable { false } else { locked };
                    if next_shared == 0 && !next_mutable {
                        restore = true;
                    } else {
                        state.set(&origin, RegState::Borrowed { shared: next_shared, mutable: next_mutable });
                    }
                }
                if restore {
                    let base = state
                        .origin_base_get(&origin)
                        .unwrap_or(RegState::Owned { kind: RegKind::Value });
                    state.origin_base_remove(&origin);
                    state.set(&origin, base);
                }
                state.set(reg, RegState::Released);
            } else if shared > 0 || mutable {
                ctx.report(
                    Trap::BorrowConflict,
                    format!("cannot release '{reg}' while it has active borrows"),
                    loc,
                    Some(reg),
                    Some("release every borrow view before releasing the source".into()),
                    false,
                );
            } else {
                state.set(reg, RegState::Released);
            }
        }
        RegState::Conflict => {}
    }
}

fn order_seen_bit(order: sa_ir::AtomicOrdering) -> u8 {
    match order {
        sa_ir::AtomicOrdering::Relaxed => 0x01,
        sa_ir::AtomicOrdering::Acquire => 0x02,
        sa_ir::AtomicOrdering::Release => 0x04,
        sa_ir::AtomicOrdering::AcqRel => 0x08,
        sa_ir::AtomicOrdering::SeqCst => 0x10,
    }
}

/// sci `sameAddressRmwCompatible` (`common/atomic.zig:92-96`): equal orderings
/// are compatible, and seq_cst mixes with everything; relaxed/acquire/release/
/// acq_rel in other combinations are not.
fn same_address_rmw_compatible(
    prev: sa_ir::AtomicOrdering,
    next: sa_ir::AtomicOrdering,
) -> bool {
    prev == next
        || prev == sa_ir::AtomicOrdering::SeqCst
        || next == sa_ir::AtomicOrdering::SeqCst
}

/// key = (base register, static offset), matching sci `atomicKey`
/// `(reg << 32) | imm_u64`. sars previously keyed only on the base name and
/// stored just the last ordering, losing both offset and mixed-order history.
fn check_atomic(
    ctx: &mut RuleCtx,
    addr: &sa_ir::Address,
    order: sa_ir::AtomicOrdering,
    loc: &SourceLoc,
) {
    let key = format!("{}::{}", addr.base, addr.offset);
    let seen = ctx.atomics.get(&key).copied().unwrap_or(0);
    for (bit, prev) in [
        (0x01u8, sa_ir::AtomicOrdering::Relaxed),
        (0x02, sa_ir::AtomicOrdering::Acquire),
        (0x04, sa_ir::AtomicOrdering::Release),
        (0x08, sa_ir::AtomicOrdering::AcqRel),
        (0x10, sa_ir::AtomicOrdering::SeqCst),
    ] {
        if seen & bit != 0 && !same_address_rmw_compatible(prev, order) {
            ctx.report(
                Trap::AtomicOrderingMismatch,
                format!(
                    "same-address RMW ordering '{}' is incompatible with '{}'",
                    prev.name(),
                    order.name()
                ),
                loc,
                None,
                None,
                false,
            );
            break;
        }
    }
    ctx.atomics.insert(key, seen | order_seen_bit(order));
}

/// Extract register-like identifiers referenced by a `$...$` native line,
/// mirroring sci `collectNativeRegisterNames` (line_classifier_parse.zig:204):
/// quoted string contents are skipped, `@`/`%`-prefixed tokens are skipped,
/// and duplicates are removed. Only names already live in `state` are acted
/// on by the caller, so stray asm keywords are harmless.
fn native_reg_names(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    let ident_start = |b: u8| b.is_ascii_alphabetic() || b == b'_';
    let ident_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if !ident_start(bytes[i]) {
            i += 1;
            continue;
        }
        let prev = if i == 0 { 0 } else { bytes[i - 1] };
        let start = i;
        i += 1;
        while i < bytes.len() && ident_char(bytes[i]) {
            i += 1;
        }
        if prev == b'@' || prev == b'%' {
            continue;
        }
        let token = &text[start..i];
        if !out.iter().any(|n| n == token) {
            out.push(token.to_string());
        }
    }
    out
}

fn mark_escaped_alloc<S: RegStateBackend>(state: &mut S, op: &Operand) {
    if let Operand::Reg(name) = op {
        if matches!(
            state.state_of(name),
            RegState::Owned {
                kind: RegKind::Alloc | RegKind::Value
            }
        ) {
            state.escaped_insert(name);
        }
    }
}

fn apply_cap<S: RegStateBackend>(ctx: &mut RuleCtx, state: &mut S, arg: &sa_ir::CallArg, loc: &SourceLoc) {
    mark_escaped_alloc(state, &arg.value);
    match arg.cap {
        sa_ir::CapPrefix::Move => consume(ctx, state, &arg.value, loc),
        sa_ir::CapPrefix::Borrow => {
            if matches!(arg.value, Operand::Reg(_)) {
                read_operand(ctx, &arg.value, state, loc);
            } else {
                add_borrow(ctx, state, &arg.value, false, None, loc);
            }
        }
        sa_ir::CapPrefix::Raw => {
            if is_constant(ctx, &arg.value) {
                return;
            }
            read_operand(ctx, &arg.value, state, loc);
            if let Operand::Reg(name) = &arg.value {
                if matches!(state.state_of(name), RegState::Borrowed { .. }) {
                    ctx.report(
                        Trap::CapabilityMismatch,
                        format!("raw argument '{name}' requires an owned register"),
                        loc,
                        Some(name),
                        None,
                        false,
                    );
                }
            }
        }
        sa_ir::CapPrefix::ByValue => read_operand(ctx, &arg.value, state, loc),
    }
}

fn handle_call<S: RegStateBackend>(
    ctx: &mut RuleCtx,
    state: &mut S,
    target: &str,
    args: &[sa_ir::CallArg],
    loc: &SourceLoc,
) -> bool {
    let signature = ctx
        .program
        .function(target)
        .map(|function| &function.sig)
        .or_else(|| ctx.program.extern_sig(target));
    if signature.is_none() {
        ctx.report(
            Trap::UnknownRegister,
            format!("unknown function '@{target}'"),
            loc,
            None,
            Some("declare the function with @name or @extern @name".into()),
            false,
        );
    }
    for (index, arg) in args.iter().enumerate() {
        apply_cap(ctx, state, arg, loc);
        if !ctx.opts.check_capabilities {
            continue;
        }
        if let Some(sig) = signature {
            if index < sig.params.len() {
                let expected = sig.params[index].cap;
                if expected != arg.cap {
                    let view_coercion = expected == CapPrefix::ByValue
                        && sig.params[index].ty == PrimType::Ptr
                        && matches!(arg.cap, CapPrefix::Borrow | CapPrefix::Raw);
                    if !view_coercion {
                        ctx.report(
                            Trap::CapabilityMismatch,
                            format!(
                                "argument {} of '@{target}' expects {} capability, found {}",
                                index + 1,
                                Capability::from_prefix(expected).name(),
                                Capability::from_prefix(arg.cap).name()
                            ),
                            loc,
                            arg.value.as_reg(),
                            None,
                            false,
                        );
                    }
                }
            } else if !sig.is_variadic {
                ctx.report(
                    Trap::CapabilityMismatch,
                    format!("too many arguments to '@{target}'"),
                    loc,
                    None,
                    None,
                    false,
                );
            }
        }
    }
    if ctx.opts.check_capabilities {
        if let Some(sig) = signature {
            if args.len() < sig.params.len() && !sig.is_variadic {
                ctx.report(
                    Trap::CapabilityMismatch,
                    format!("missing argument {} in call to '@{target}'", sig.params.len()),
                    loc,
                    None,
                    None,
                    false,
                );
            }
        }
    }
    signature.map(|sig| sig.return_fallible).unwrap_or(false)
}

fn check_leaks<S: RegStateBackend>(ctx: &mut RuleCtx, state: &S, loc: &SourceLoc) {
    check_leaks_with(ctx, state, loc, Trap::MemoryLeak);
}

/// Same live-allocation scan as function exit, but with a caller-chosen trap.
/// sci `armTry` uses `.early_return_leak` when `?`/`early_return` would leave
/// live allocations behind on the error path.
fn check_leaks_with<S: RegStateBackend>(
    ctx: &mut RuleCtx,
    state: &S,
    loc: &SourceLoc,
    trap: Trap,
) {
    if !ctx.opts.check_leaks {
        return;
    }
    let mut leaked: Vec<String> = state
        .regs_snapshot()
        .iter()
        .filter(|(name, _reg_state)| {
            if state.escaped_contains(name) {
                return false;
            }
            // B9 batch B: sci `checkExitLeaks` mask+flags scan
            // (DELIVERY/B9_DESIGN.md section 3, items #0-#8). The mask
            // decides what is live; the batch-A flags side table decides
            // the exemptions. The old approximations (owned-alloc fast
            // path, owned-pointer-param special case, indirect-dispatch
            // special case) are subsumed by #0/#7/#8 and are gone.
            let mask = state.mask_of(name);
            if mask == bit::UNINITIALIZED
                || mask & bit::CONSUMED != 0
                || mask & bit::UNTRACKED != 0
            {
                return false; // #0 uninitialized / consumed / untracked
            }
            let flags = state.flag_get(name);
            if flags & reg_flag::CONST_DATA != 0 {
                return false; // #1 const data
            }
            if flags & reg_flag::EPHEMERAL != 0 {
                return false; // #2 ephemeral scalar temp
            }
            if flags & reg_flag::RAW_PTR != 0 {
                return false; // #3 raw pointer, no ownership
            }
            if mask & bit::BORROW_VIEW != 0 {
                // #4/#6 via origin: a borrow view of const data or of a
                // stack allocation owns nothing. A view of an owned
                // origin stays live (sci's move-while-borrowed backstop).
                if let Some(origin) = state.borrow_origin_get(name) {
                    let oflags = state.flag_get(&origin);
                    if oflags & reg_flag::CONST_DATA != 0 || oflags & reg_flag::STACK != 0 {
                        return false;
                    }
                }
            }
            if flags & reg_flag::BRANCH_COND != 0 {
                return false; // #5 branch condition temp
            }
            if flags & reg_flag::STACK != 0 {
                return false; // #6 stack allocation
            }
            if state.is_consumed_in_function(name) {
                return false; // #7 consumed inside the function
            }
            mask & bit::ACTIVE != 0 // #8 owned-active remainder leaks
        })
        .map(|(name, _)| name.clone())
        .collect();
    leaked.sort();
    for name in leaked {
        let (message, hint) = if trap == Trap::EarlyReturnLeak {
            (
                format!("early return would leak allocation '{name}'"),
                format!("release '{name}' before the early return"),
            )
        } else {
            (
                format!("allocation '{name}' is still live at function exit"),
                format!("add '!{name}' before return"),
            )
        };
        ctx.report(
            trap,
            message,
            loc,
            Some(&name),
            Some(hint),
            false,
        );
    }
}

fn mark_consumed<S: RegStateBackend>(state: &mut S, op: &Operand) {
    if let Operand::Reg(name) = op {
        state.consumed_mark(name);
    }
}

/// B9 batch A: maintain the sci `flags[]` / `consumed_in_function[]` side
/// tables (DELIVERY/B9_DESIGN.md sections 2 and 9.4). Nothing in batch A
/// reads these tables for a judgment; they are infrastructure so batch B
/// can switch the exit scan to mask+flags without another structural pass.
/// Writes run after the transfer match, observing the post-`define` state,
/// exactly like sci arms set flags after `assignValue`.
fn apply_reg_flags<S: RegStateBackend>(ins: &Instruction, state: &mut S) {
    let dests: Vec<&str> = ins.all_dests().collect();
    match &ins.kind {
        InstructionKind::StackAlloc { .. } => {
            for dest in &dests {
                state.flag_replace(dest, reg_flag::STACK);
            }
        }
        InstructionKind::Load { .. } => {
            // sci armLoad: dest flag is the loaded type — ptr is a raw
            // pointer, anything else an ephemeral scalar (load's default
            // type is i64 when no `as` annotation is present).
            let byte = if ins.ty == Some(PrimType::Ptr) {
                reg_flag::RAW_PTR
            } else {
                reg_flag::EPHEMERAL
            };
            for dest in &dests {
                state.flag_replace(dest, byte);
            }
        }
        InstructionKind::Take { .. } | InstructionKind::Op { .. } => {
            // sci armLoad (take) and armOp both leave the dest flag at 0.
            for dest in &dests {
                state.flag_replace(dest, 0);
            }
        }
        InstructionKind::Assign { src } => {
            // sci armAssign: propagate RAW_PTR from a register source;
            // an ephemeral source (literal or EPHEMERAL-flagged register)
            // replaces the dest byte with EPHEMERAL.
            let (src_flags, src_ephemeral) = match src {
                Operand::Reg(name) => {
                    let flags = state.flag_get(name);
                    (flags, flags & reg_flag::EPHEMERAL != 0)
                }
                Operand::Int(_) | Operand::Float(_) => (0, true),
                _ => (0, false),
            };
            for dest in &dests {
                if src_flags & reg_flag::RAW_PTR != 0 {
                    state.flag_set(dest, reg_flag::RAW_PTR);
                }
                if src_ephemeral {
                    state.flag_replace(dest, reg_flag::EPHEMERAL);
                }
            }
        }
        InstructionKind::Call { .. }
        | InstructionKind::CallIndirect { .. }
        | InstructionKind::Try { .. }
        | InstructionKind::Cmpxchg { .. } => {
            // sci armCall / armTry / armCmpxchg reset the dest flag to 0.
            for dest in &dests {
                state.flag_replace(dest, 0);
            }
        }
        InstructionKind::AtomicLoad { .. } | InstructionKind::AtomicRmw { .. } => {
            for dest in &dests {
                state.flag_replace(dest, reg_flag::EPHEMERAL);
            }
        }
        InstructionKind::VaStart => {
            for dest in &dests {
                state.flag_replace(dest, reg_flag::RAW_PTR);
            }
        }
        InstructionKind::VaArg { .. } => {
            let byte = if ins.ty == Some(PrimType::Ptr) {
                reg_flag::RAW_PTR
            } else {
                reg_flag::EPHEMERAL
            };
            for dest in &dests {
                state.flag_replace(dest, byte);
            }
        }
        InstructionKind::RawCast { src } | InstructionKind::AssumeSafe { src } => {
            // sci: a safe pointer reinterpreted from a raw pointer keeps
            // the raw flag, like assign propagation.
            if let Operand::Reg(name) = src {
                if state.flag_get(name) & reg_flag::RAW_PTR != 0 {
                    for dest in &dests {
                        state.flag_set(dest, reg_flag::RAW_PTR);
                    }
                }
            }
        }
        InstructionKind::Borrow { .. } | InstructionKind::AssumeBorrow { .. } => {
            // sci isStackAllocated / isConstData resolve through the
            // borrow origin; batch A materialises that by inheriting the
            // origin's STACK / CONST_DATA bits onto the view (design 9.4).
            // The owned-source view keeps no exemption bit, preserving
            // the move-while-borrowed backstop.
            for dest in &dests {
                if let Some(origin) = state.borrow_origin_get(dest) {
                    let inherited =
                        state.flag_get(&origin) & (reg_flag::STACK | reg_flag::CONST_DATA);
                    if inherited != 0 {
                        state.flag_set(dest, inherited);
                    }
                }
            }
        }
        InstructionKind::Br { cond, .. } | InstructionKind::BrNull { cond, .. } => {
            if let Operand::Reg(name) = cond {
                state.flag_set(name, reg_flag::BRANCH_COND);
            }
        }
        _ => {}
    }
    // consumed_in_function: sci collects this syntactically over the whole
    // function body (markInstructionConsumedRegs); batch A records the
    // same events at transfer time and the CFG merge unions them.
    match &ins.kind {
        InstructionKind::Move { src }
        | InstructionKind::Assign { src }
        | InstructionKind::Try { src } => mark_consumed(state, src),
        InstructionKind::Release { reg } => state.consumed_mark(reg),
        InstructionKind::Return { value } | InstructionKind::EarlyReturn { value } => {
            if let Some(op) = value {
                mark_consumed(state, op);
            }
        }
        InstructionKind::Transfer { value, .. } => mark_consumed(state, value),
        InstructionKind::Call { args, .. } | InstructionKind::CallIndirect { args, .. } => {
            for arg in args {
                if arg.cap == CapPrefix::Move {
                    mark_consumed(state, &arg.value);
                }
            }
        }
        _ => {}
    }
}

pub fn transfer<S: RegStateBackend>(ins: &Instruction, state: &mut S, ctx: &mut RuleCtx) {
    let loc = &ins.loc;
    let plan = crate::runtime_mask::plan(ins, state);
    let errors_before = ctx.sink.errors.len();
    // B2 / sci armRawCast+armAssumeSafe+armAssumeBorrow: the FFI airlock.
    // These unsafe escapes are only legal inside an @ffi_wrapper function.
    let unsafe_escape = matches!(
        ins.kind,
        InstructionKind::RawCast { .. }
            | InstructionKind::AssumeSafe { .. }
            | InstructionKind::AssumeBorrow { .. }
    );
    if unsafe_escape && ctx.signature.kind != FunctionKind::FfiWrapper {
        ctx.report(
            Trap::IllegalUnsafeContext,
            "raw pointer and assume_* instructions are only legal inside @ffi_wrapper",
            loc,
            None,
            None,
            false,
        );
        return;
    }
    match &ins.kind {
        InstructionKind::Alloc { .. } | InstructionKind::StackAlloc { .. } => {
            for operand in ins.reads() {
                read_operand(ctx, operand, state, loc);
            }
            let kind = if matches!(ins.kind, InstructionKind::StackAlloc { .. }) {
                RegKind::StackAlloc
            } else {
                RegKind::Alloc
            };
            define(ctx, ins, state, RegState::Owned { kind }, loc);
        }
        InstructionKind::Assign { src } => {
            let scalar = is_scalar_operand(ctx, state, src);
            consume_assign(ctx, state, src, loc);
            let kind = if scalar { RegKind::Scalar } else { RegKind::Value };
            define(ctx, ins, state, RegState::Owned { kind }, loc);
        }
        InstructionKind::Op { .. } | InstructionKind::Load { .. } | InstructionKind::Take { .. } => {
            for operand in ins.reads() {
                read_operand(ctx, operand, state, loc);
            }
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Scalar }, loc);
        }
        InstructionKind::Move { src } => {
            consume(ctx, state, src, loc);
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
            if let (Some(dest), Operand::Reg(source)) = (ins.dest.as_deref(), src) {
                if let Some(fallible) = state.fallible_get(source) {
                    state.fallible_insert(dest, fallible);
                }
            }
        }
        InstructionKind::Transfer { container, value } => {
            // sci armTransfer (verify_body_arms_alloc_borrow.zig:251-302):
            // `transfer <container>, <value>` — ownership of `value` moves
            // into `container`. Verifier-visible annotation; the preceding
            // `store` already did the data move. Check order below mirrors
            // sci verbatim: borrowed-container -> immutable -> consumed ->
            // active borrows, then the value retires like a `^` move-out
            // and the container gains the boolean owns_children bit.
            let (container_reg, _) = match (container, value) {
                (Operand::Reg(c), Operand::Reg(_)) => (c, ()),
                _ => {
                    ctx.report(
                        Trap::TransferIntoBorrow,
                        "transfer requires register operands",
                        loc,
                        None,
                        None,
                        false,
                    );
                    return;
                }
            };
            // Destination must be a live OWNED root. Transferring into a
            // borrow view / ffi borrow / interior pointer would leak the
            // value when the borrow ends. (sci also walks the interior
            // tree here; in sars every interior pointer carries the
            // INTERIOR_PTR bit, so the mask test covers it.)
            let cmask = state.mask_of(container_reg);
            if cmask & (bit::BORROW_VIEW | bit::FFI_BORROW | bit::INTERIOR_PTR) != 0 {
                ctx.report(
                    Trap::TransferIntoBorrow,
                    "cannot transfer ownership into a borrowed container",
                    loc,
                    Some(container_reg),
                    None,
                    false,
                );
                return;
            }
            // sci isImmutableConst: an @const hard constant (flag) OR a
            // Rust-style immutable binding (capability bit 0x0100). sars
            // keeps the bit in the mask and named @const symbols in
            // program.consts.
            if cmask & bit::IMMUTABLE != 0 || is_named_const(ctx, container).is_some() {
                ctx.report(
                    Trap::TransferIntoImmutable,
                    "cannot transfer ownership into an immutable container",
                    loc,
                    Some(container_reg),
                    None,
                    false,
                );
                return;
            }
            // B5-fix (#19691): align check order with sci armTransfer
            // (verify_body_arms_alloc_borrow.zig:276-281). sci tests the
            // ACTIVE bit BEFORE hasActiveBorrowRefs, so a container whose
            // mask lost ACTIVE reports use_after_move 1009, not
            // borrow_conflict. sars models a borrow as LOCKED_* without
            // ACTIVE, so this fixes case8b/case9 (0x02/0x04): E1008 ->
            // E1009, matching sci's oracle (actual_mask 2/4, trap 1009).
            if cmask & bit::ACTIVE == 0 {
                ctx.report(
                    Trap::UseAfterMove,
                    "cannot transfer into a consumed container",
                    loc,
                    Some(container_reg),
                    None,
                    false,
                );
                return;
            }
            // The active|locked combination (ACTIVE present together with
            // LOCKED bits) still maps to sci's hasActiveBorrowRefs ->
            // borrow_conflict. Kept for that state; does not fire for the
            // LOCKED-without-ACTIVE case8b/case9 above (now use_after_move).
            if cmask & (bit::LOCKED_READ | bit::LOCKED_MUT) != 0 {
                ctx.report(
                    Trap::BorrowConflict,
                    "container has active borrows",
                    loc,
                    Some(container_reg),
                    None,
                    false,
                );
                return;
            }
            // Value retires exactly like a `^` move-out, so the
            // function-exit leak check is satisfied.
            consume(ctx, state, value, loc);
            // Record the obligation on the container with a single boolean
            // owns_children bit (re-read after the consume so a
            // self-transfer `transfer x, x` keeps the consumed state).
            let cmask = state.mask_of(container_reg);
            state.set_mask(container_reg, cmask | bit::OWNS_CHILDREN);
        }
        InstructionKind::Store { addr, value } => {
            for operand in ins.reads() {
                read_operand(ctx, operand, state, loc);
            }
            mark_escaped_alloc(state, &addr.base);
            mark_escaped_alloc(state, value);
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
        }
        InstructionKind::Release { reg } => release(ctx, state, reg, loc),
        InstructionKind::Borrow { src, mutable } | InstructionKind::AssumeBorrow { src, mutable } => {
            if let Some(dest) = ins.dest.as_deref() {
                check_redefine(ctx, state, dest, loc);
                state.set(
                    dest,
                    RegState::Borrowed {
                        shared: 0,
                        mutable: false,
                    },
                );
            }
            add_borrow(ctx, state, src, *mutable, ins.dest.as_deref(), loc);
        }
        InstructionKind::Call { target, args } => {
            let fallible = handle_call(ctx, state, target, args, loc);
            let scalar_result = ctx
                .program
                .function(target)
                .map(|function| &function.sig)
                .or_else(|| ctx.program.extern_sig(target))
                .map(|sig| {
                    !matches!(
                        sig.return_ty,
                        PrimType::Ptr
                            | PrimType::BlobHandle
                            | PrimType::V128
                            | PrimType::Void
                    )
                })
                .unwrap_or(false);
            let kind = if scalar_result { RegKind::Scalar } else { RegKind::Value };
            define(ctx, ins, state, RegState::Owned { kind }, loc);
            if let Some(dest) = ins.dest.as_deref() {
                state.fallible_insert(dest, fallible);
            }
        }
        InstructionKind::CallIndirect { target, args } => {
            read_operand(ctx, target, state, loc);
            for arg in args {
                apply_cap(ctx, state, arg, loc);
            }
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
        }
        InstructionKind::Return { value } => {
            if let Some(operand) = value {
                read_operand(ctx, operand, state, loc);
                // sci armReturn: a fallible value may only be returned from a
                // fallible function; otherwise it must be propagated with `?`.
                if let Operand::Reg(name) = operand {
                    if state.fallible_get(name) == Some(true)
                        && !ctx.signature.return_fallible
                    {
                        ctx.report(
                            Trap::FallibleContractMismatch,
                            format!(
                                "fallible value '{name}' must be propagated with ? or returned from a fallible function"
                            ),
                            loc,
                            Some(name),
                            None,
                            false,
                        );
                    }
                }
                let by_value = !matches!(
                    ctx.signature.return_cap,
                    Some(sa_ir::CapPrefix::Borrow) | Some(sa_ir::CapPrefix::Raw)
                );
                // sci armReturn (verify_body_arms_call_return.zig:303-305): a stack
                // allocation cannot escape its defining function by move. Only
                // applies to the non-fallible, by-value path, matching sci's
                // `else` branch after the fallible contract check.
                if let Operand::Reg(name) = operand {
                    if by_value
                        && state.fallible_get(name) != Some(true)
                        && matches!(
                            state.state_of(name),
                            RegState::Owned {
                                kind: RegKind::StackAlloc
                            }
                        )
                    {
                        ctx.report(
                            Trap::StackEscape,
                            "stack allocation cannot be returned",
                            loc,
                            Some(name),
                            None,
                            false,
                        );
                    }
                }
                if by_value {
                    // sci armReturn: immutable registers cannot be returned by move.
                    const_mutation(ctx, operand, "returned by move", loc);
                    if let Operand::Reg(name) = operand {
                        state.set(name, RegState::Moved);
                    }
                }
            }
            // B9 batch B: the return operand joins consumed_in_function
            // before the exit scan (sci collects it syntactically over
            // the whole body; batch A's post-transfer write in
            // apply_reg_flags lands after this scan). Without this, a
            // returned allocation/view is reported as a leak.
            if let Some(Operand::Reg(name)) = value {
                state.consumed_mark(name);
            }
            check_leaks(ctx, state, loc);
        }
        InstructionKind::EarlyReturn { value } => {
            // sci verify_body.zig:511-513 routes early_return through armTry,
            // which scans for live allocations and reports early_return_leak.
            if let Some(operand) = value {
                read_operand(ctx, operand, state, loc);
                // B9 batch B: same consumed-before-scan timing as the
                // Return arm above (apply_reg_flags records
                // Return|EarlyReturn operands as consumed).
                if let Operand::Reg(name) = operand {
                    state.consumed_mark(name);
                }
            }
            check_leaks_with(ctx, state, loc, Trap::EarlyReturnLeak);
        }
        InstructionKind::AtomicRmw { addr, value, order, .. } => {
            check_atomic(ctx, addr, *order, loc);
            read_operand(ctx, &addr.base, state, loc);
            read_operand(ctx, value, state, loc);
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
        }
        InstructionKind::Cmpxchg { addr, order, .. } => {
            // sci armCmpxchg: same-address RMW ordering history applies to
            // cmpxchg too; previously sars left it on the default arm with no
            // atomic-order check.
            check_atomic(ctx, addr, *order, loc);
            for operand in ins.reads() {
                read_operand(ctx, operand, state, loc);
            }
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
        }
        InstructionKind::Try { src } => {
            read_operand(ctx, src, state, loc);
            // sci armTry: `(src & fallible) == 0` is a contract mismatch. A
            // register is only fallible when a call produced one (Some(true));
            // unknown/never-set registers are not fallible either.
            if let Operand::Reg(name) = src {
                if state.fallible_get(name) != Some(true) {
                    ctx.report(
                        Trap::FallibleContractMismatch,
                        format!("'?' can only be applied to a fallible value, but '{name}' is not fallible"),
                        loc,
                        Some(name),
                        None,
                        false,
                    );
                }
            }
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
            if let Some(dest) = ins.dest.as_deref() {
                state.fallible_insert(dest, false);
            }
        }
        InstructionKind::Native { text } => {
            // sci armNative (verify_body_arms_assign_branch.zig:141-151) walks
            // every referenced register and consumes it at a contract boundary
            // (clear_borrow_ctx.zig:131-153): reject stack allocation escapes,
            // then move-consume. sars previously fell through to the default
            // arm (read-only), so native escape neither consumed nor rejected
            // stack allocations.
            for name in native_reg_names(text) {
                match state.state_of(&name) {
                    RegState::Undefined | RegState::Conflict => continue,
                    RegState::Owned { kind: RegKind::StackAlloc } => {
                        ctx.report(
                            Trap::StackEscape,
                            "stack allocation cannot cross a native escape boundary",
                            loc,
                            Some(&name),
                            None,
                            false,
                        );
                    }
                    _ => consume(ctx, state, &Operand::Reg(name.clone()), loc),
                }
            }
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
        }
        _ => {
            for operand in ins.reads() {
                read_operand(ctx, operand, state, loc);
            }
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
        }
    }
    apply_reg_flags(ins, state);
    if let Some(plan) = plan {
        let bypass_errored = ctx.sink.errors.len() > errors_before;
        if let Some(decision) = crate::runtime_mask::apply(state, &plan, bypass_errored) {
            if let Some(trap) = decision.confirmed_trap {
                ctx.report(
                    trap,
                    format!(
                        "mask lookup rejects {:?} on register '{}' (prev mask 0x{:04x})",
                        plan.op, plan.reg, plan.prev_mask
                    ),
                    loc,
                    Some(&plan.reg),
                    None,
                    false,
                );
            }
        }
    }
}
