//! E1 (first batch): static out-of-bounds pointer-offset check.
//!
//! Port of the narrow, fully-static slice of sci's
//! `checkStaticOffsetInBounds` (snapshot_merge_state_in_scope.zig:334-366):
//! when an allocation's size and a pointer access's byte offset are both
//! compile-time constants, an access at or past the end is a
//! `BorrowConflict` ("pointer offset exceeds allocation size").
//!
//! Dynamic offsets/sizes, negative offsets and pointers of unknown provenance
//! stay permissive (sci does the same: `static_offset orelse return null`,
//! `!alloc_size_known orelse return null`). Interior-tree field identity and
//! overlapping-borrow checks are intentionally left to a later batch.

use crate::rules::RuleCtx;
use crate::state::RegStateBackend;
use sa_ir::{Instruction, InstructionKind, Operand, Trap};
use std::collections::HashMap;

/// `root` is the register holding the known-size Allocation; `off` is the
/// accumulated static byte offset from that root.
type View = (String, i64);

fn int_const(op: &Operand) -> Option<i64> {
    match op {
        Operand::Int(n) => Some(*n as i64),
        _ => None,
    }
}

fn resolve_view<'a>(
    views: &'a HashMap<String, View>,
    alloc_size: &HashMap<String, i64>,
    op: &Operand,
    extra_off: i64,
) -> Option<(String, i64, i64)> {
    if let Operand::Reg(reg) = op {
        if let Some((root, off)) = views.get(reg) {
            if let Some(size) = alloc_size.get(root) {
                return Some((root.clone(), off.saturating_add(extra_off), *size));
            }
        }
        // Direct access off the allocation root itself.
        if let Some(size) = alloc_size.get(reg) {
            return Some((reg.clone(), extra_off, *size));
        }
    }
    None
}

fn check_access<S: RegStateBackend>(
    ctx: &mut RuleCtx,
    views: &HashMap<String, View>,
    alloc_size: &HashMap<String, i64>,
    base: &Operand,
    off: i64,
    ins: &Instruction,
) {
    if off < 0 {
        return;
    }
    if let Some((_root, total, size)) = resolve_view(views, alloc_size, base, off) {
        // Strict containment: total >= size is one-past-the-end or beyond and
        // denotes a real access here, so it is an error (matches sci).
        if total >= size {
            ctx.report(
                Trap::BorrowConflict,
                "pointer offset exceeds allocation size",
                &ins.loc,
                match base {
                    Operand::Reg(r) => Some(r),
                    _ => None,
                },
                Some("shrink the constant offset or widen the allocation".into()),
                false,
            );
        }
    }
}

/// Linear, intra-function pre-pass. Runs independently of the dataflow
/// backend so it needs no changes to `RegStateBackend`/merge.
pub fn check_static_bounds<S: RegStateBackend>(
    function: &sa_ir::Function,
    ctx: &mut RuleCtx,
) {
    let mut alloc_size: HashMap<String, i64> = HashMap::new();
    let mut views: HashMap<String, View> = HashMap::new();

    for ins in function.all_instructions() {
        match &ins.kind {
            InstructionKind::Alloc { size } | InstructionKind::StackAlloc { size } => {
                if let (Some(dest), Some(n)) = (ins.dest.as_deref(), int_const(size)) {
                    if n >= 0 {
                        alloc_size.insert(dest.to_string(), n);
                    }
                }
            }
            InstructionKind::PtrAdd { base, offset } => {
                if let Some(o) = int_const(offset) {
                    check_access::<S>(ctx, &views, &alloc_size, base, o, ins);
                }
                if let (Some(dest), Operand::Reg(base_reg)) = (ins.dest.as_deref(), base) {
                    let o = int_const(offset).unwrap_or(0);
                    if let Some((root, off)) = views.get(base_reg).cloned() {
                        views.insert(dest.to_string(), (root, off.saturating_add(o)));
                    } else if alloc_size.contains_key(base_reg) {
                        views.insert(dest.to_string(), (base_reg.clone(), o));
                    }
                }
            }
            InstructionKind::Load { addr } | InstructionKind::Take { addr } => {
                check_access::<S>(ctx, &views, &alloc_size, &addr.base, addr.offset, ins);
                // NOTE: a plain load yields an independent pointer VALUE, not an
                // interior view of the base allocation. sci only attaches an
                // interior child when the source carries a borrow_view /
                // interior_ptr flag (armLoad, verify_body_arms_alloc_borrow.zig:75).
                // Propagating the base view here caused false positives on
                // vtable loads (rosetta 131/164/776); do not do it.
            }
            InstructionKind::Store { addr, .. } => {
                check_access::<S>(ctx, &views, &alloc_size, &addr.base, addr.offset, ins);
            }
            InstructionKind::AtomicLoad { addr, .. }
            | InstructionKind::AtomicStore { addr, .. }
            | InstructionKind::Cmpxchg { addr, .. }
            | InstructionKind::AtomicRmw { addr, .. } => {
                check_access::<S>(ctx, &views, &alloc_size, &addr.base, addr.offset, ins);
            }
            InstructionKind::Move { src } => {
                if let (Some(dest), Operand::Reg(src_reg)) = (ins.dest.as_deref(), src) {
                    if let Some(v) = views.get(src_reg).cloned() {
                        views.insert(dest.to_string(), v);
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{Block, Function, Instruction, Signature, SourceLoc};

    fn loc(line: u32) -> SourceLoc {
        SourceLoc::new("test.sa", line, 1)
    }
    fn ins(kind: InstructionKind, line: u32) -> Instruction {
        Instruction::new(kind, loc(line), "")
    }
    fn run(kinds: Vec<Instruction>) -> Vec<sa_ir::Diagnostic> {
        let f = Function::new(Signature::new("main"), loc(1));
        // Build a single block directly (avoid needing Block builder details).
        let mut f = f;
        let mut b = Block::new("L_entry", loc(1));
        b.instrs = kinds;
        f.blocks.push(b);
        let program = sa_ir::Program::new();
        let mut program = program;
        program.functions.push(f);
        let mut sink = crate::DiagSink::default();
        let opts = crate::VerifyOptions::default();
        let mut ctx = RuleCtx {
            program: &program,
            opts: &opts,
            sink: &mut sink,
            func_name: "main",
            signature: &program.functions[0].sig,
            atomics: &mut HashMap::new(),
        };
        check_static_bounds::<crate::state::FnState>(&program.functions[0], &mut ctx);
        sink.errors
    }

    #[test]
    fn oob_static_offset_is_rejected() {
        // buf = alloc 16; x = load buf+16 -> BorrowConflict (E1008)
        let mut a = ins(InstructionKind::Alloc { size: Operand::Int(16) }, 2);
        a.dest = Some("buf".into());
        let mut l = ins(
            InstructionKind::Load {
                addr: sa_ir::Address::new(Operand::Reg("buf".into()), 16),
            },
            3,
        );
        l.dest = Some("x".into());
        let errs = run(vec![a, l]);
        assert!(errs.iter().any(|d| d.trap == Trap::BorrowConflict
            && d.message.contains("pointer offset exceeds allocation size")));
    }

    #[test]
    fn inbounds_static_offset_is_allowed() {
        let mut a = ins(InstructionKind::Alloc { size: Operand::Int(16) }, 2);
        a.dest = Some("buf".into());
        let mut l = ins(
            InstructionKind::Load {
                addr: sa_ir::Address::new(Operand::Reg("buf".into()), 12),
            },
            3,
        );
        l.dest = Some("x".into());
        assert!(run(vec![a, l]).is_empty());
    }

    #[test]
    fn dynamic_offset_is_permissive() {
        // Unknown offset must not be flagged (matches sci: static_offset orelse return null).
        let mut a = ins(InstructionKind::Alloc { size: Operand::Int(16) }, 2);
        a.dest = Some("buf".into());
        let mut p = ins(
            InstructionKind::PtrAdd {
                base: Operand::Reg("buf".into()),
                offset: Operand::Reg("i".into()),
            },
            3,
        );
        p.dest = Some("q".into());
        let mut l = ins(
            InstructionKind::Load {
                addr: sa_ir::Address::new(Operand::Reg("q".into()), 0),
            },
            4,
        );
        l.dest = Some("x".into());
        assert!(run(vec![a, p, l]).is_empty());
    }
}
