use crate::state::{FnState, RegKind, RegState};
use crate::{DiagSink, VerifyOptions};
use sa_ir::{
    Capability, Diagnostic, Instruction, InstructionKind, Operand, Program, Signature, SourceLoc,
    Trap,
};
use std::collections::HashMap;

pub struct RuleCtx<'a> {
    pub program: &'a Program,
    pub opts: &'a VerifyOptions,
    pub sink: &'a mut DiagSink,
    pub func_name: &'a str,
    pub signature: &'a Signature,
    pub atomics: &'a mut HashMap<String, sa_ir::AtomicOrdering>,
}

impl<'a> RuleCtx<'a> {
    fn report(
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

fn check_redefine(ctx: &mut RuleCtx, state: &FnState, reg: &str, loc: &SourceLoc) {
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

fn define(ctx: &mut RuleCtx, ins: &Instruction, state: &mut FnState, new_state: RegState, loc: &SourceLoc) {
    for dest in ins.all_dests() {
        let self_update = matches!(state.state_of(dest), RegState::Owned { .. })
            && ins.reads().iter().any(|op| matches!(op, Operand::Reg(name) if name == dest));
        if !self_update {
            check_redefine(ctx, state, dest, loc);
        }
        state.set(dest, new_state.clone());
    }
}

fn is_constant(ctx: &RuleCtx, op: &Operand) -> bool {
    match op {
        Operand::Reg(name) => ctx.program.consts.iter().any(|c| c.name == *name),
        _ => op.is_const(),
    }
}

fn read_operand(ctx: &mut RuleCtx, op: &Operand, state: &FnState, loc: &SourceLoc) {
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
            if !state.borrow_origin.contains_key(name) {
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

fn consume(ctx: &mut RuleCtx, state: &mut FnState, op: &Operand, loc: &SourceLoc) {
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

fn is_scalar_operand(_ctx: &RuleCtx, state: &FnState, op: &Operand) -> bool {
    match op {
        Operand::Reg(name) => matches!(
            state.state_of(name),
            RegState::Owned { kind: RegKind::Scalar }
        ),
        _ => !matches!(op, Operand::Symbol(_) | Operand::Label(_)),
    }
}

fn consume_assign(ctx: &mut RuleCtx, state: &mut FnState, op: &Operand, loc: &SourceLoc) {
    if let Operand::Reg(name) = op {
        if matches!(state.state_of(name), RegState::Owned { kind: RegKind::Param }) {
            read_operand(ctx, op, state, loc);
            return;
        }
    }
    consume(ctx, state, op, loc);
}

fn add_borrow(
    ctx: &mut RuleCtx,
    state: &mut FnState,
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
            state.origin_base.entry(name.to_string()).or_insert(owned);
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
        state.borrow_origin.insert(dest.to_string(), name.to_string());
    }
}

fn release(ctx: &mut RuleCtx, state: &mut FnState, reg: &str, loc: &SourceLoc) {
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
            state.origin_base.remove(reg);
            state.set(reg, RegState::Released);
        }
        RegState::Borrowed { shared, mutable } => {
            if let Some(origin) = state.borrow_origin.remove(reg) {
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
                        .origin_base
                        .get(&origin)
                        .cloned()
                        .unwrap_or(RegState::Owned { kind: RegKind::Value });
                    state.origin_base.remove(&origin);
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

fn check_atomic(ctx: &mut RuleCtx, addr: &str, order: sa_ir::AtomicOrdering, loc: &SourceLoc) {
    if let Some(previous) = ctx.atomics.get(addr).copied() {
        if previous != order {
            ctx.report(
                Trap::AtomicOrderingMismatch,
                format!(
                    "same-address RMW ordering changed from '{}' to '{}'",
                    previous.name(),
                    order.name()
                ),
                loc,
                None,
                None,
                false,
            );
        }
    }
    ctx.atomics.insert(addr.to_string(), order);
}

fn mark_escaped_alloc(state: &mut FnState, op: &Operand) {
    if let Operand::Reg(name) = op {
        if matches!(state.state_of(name), RegState::Owned { kind: RegKind::Alloc }) {
            state.escaped.insert(name.clone());
        }
    }
}

fn apply_cap(ctx: &mut RuleCtx, state: &mut FnState, arg: &sa_ir::CallArg, loc: &SourceLoc) {
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

fn handle_call(
    ctx: &mut RuleCtx,
    state: &mut FnState,
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

fn check_leaks(ctx: &mut RuleCtx, state: &FnState, loc: &SourceLoc) {
    if !ctx.opts.check_leaks {
        return;
    }
    let indirect_dispatch = ctx
        .program
        .functions
        .iter()
        .find(|function| function.name() == ctx.func_name)
        .map(|function| {
            function
                .all_instructions()
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::CallIndirect { .. }))
        })
        .unwrap_or(false);
    let mut leaked: Vec<String> = state
        .regs
        .iter()
        .filter(|(name, reg_state)| {
            if state.escaped.contains(*name) {
                return false;
            }
            if matches!(reg_state, RegState::Owned { kind: RegKind::Alloc }) {
                return true;
            }
            indirect_dispatch
                && state.borrow_params.contains(*name)
                && !matches!(reg_state, RegState::Moved | RegState::Released)
        })
        .map(|(name, _)| name.clone())
        .collect();
    leaked.sort();
    for name in leaked {
        ctx.report(
            Trap::MemoryLeak,
            format!("allocation '{name}' is still live at function exit"),
            loc,
            Some(&name),
            Some(format!("add '!{name}' before return")),
            false,
        );
    }
}

pub fn transfer(ins: &Instruction, state: &mut FnState, ctx: &mut RuleCtx) {
    let loc = &ins.loc;
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
                if let Some(fallible) = state.fallible.get(source).copied() {
                    state.fallible.insert(dest.to_string(), fallible);
                }
            }
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
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
            if let Some(dest) = ins.dest.as_deref() {
                state.fallible.insert(dest.to_string(), fallible);
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
                let by_value = !matches!(
                    ctx.signature.return_cap,
                    Some(sa_ir::CapPrefix::Borrow) | Some(sa_ir::CapPrefix::Raw)
                );
                if by_value {
                    if let Operand::Reg(name) = operand {
                        state.set(name, RegState::Moved);
                    }
                }
            }
            check_leaks(ctx, state, loc);
        }
        InstructionKind::AtomicRmw { addr, value, order, .. } => {
            check_atomic(ctx, &addr.base.to_string(), *order, loc);
            read_operand(ctx, &addr.base, state, loc);
            read_operand(ctx, value, state, loc);
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
        }
        InstructionKind::Try { src } => {
            read_operand(ctx, src, state, loc);
            if let Operand::Reg(name) = src {
                if state.fallible.get(name) == Some(&false) {
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
                state.fallible.insert(dest.to_string(), false);
            }
        }
        _ => {
            for operand in ins.reads() {
                read_operand(ctx, operand, state, loc);
            }
            define(ctx, ins, state, RegState::Owned { kind: RegKind::Value }, loc);
        }
    }
}
