//! E1: interior-pointer checks, ported as linear intra-function pre-passes
//! that run independently of the dataflow backend (no RegStateBackend/merge
//! changes).
//!
//! batch 1 — `check_static_bounds`: static out-of-bounds pointer-offset check
//! (sci `checkStaticOffsetInBounds`, snapshot_merge_state_in_scope.zig:334-366).
//! When an allocation's size and a pointer access's byte offset are both
//! compile-time constants, an access at or past the end is a `BorrowConflict`
//! ("pointer offset exceeds allocation size").
//!
//! batch 2 — `check_overlapping_borrows`: overlapping interior-field mutable
//! borrow check (sci `hasOverlappingInteriorBorrow`,
//! snapshot_merge_state_in_scope.zig:405-414 plus `armBorrow` in
//! verify_body_arms_alloc_borrow.zig:211-239). Field identity is
//! (root, accumulated static offset) established by `ptr_add`
//! (`setInteriorOffsetFromBase`); a `&mut` of a field while a *different*
//! register holds a borrow on the same field is `ReadWriteConflict`
//! ("cannot borrow mut while overlapping field borrows are active").
//!
//! Dynamic offsets/sizes, negative offsets and pointers of unknown
//! provenance stay permissive (sci: `static_offset orelse return null`;
//! unknown offset in `sameInteriorField` is treated conservatively, but sars
//! errs on the side of not reporting when field identity is not statically
//! known, to avoid false positives on the 968-name corpus).
//!
//! E4 (store write check) — `check_shared_borrow_writes`: a `store` through a
//! borrow view whose origin has more than one live shared borrow is a
//! `ReadWriteConflict` ("cannot write through a shared borrow"), mirroring
//! sci's `writeCheck` (collect_metadata.zig:477-514) invoked from `armStore`.

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

/// A live borrow on a source register: (mutable, field identity). Mirrors
/// sci's `locks[src]` + interior root/offset for that source.
type BorrowLock = (bool, View);

/// One interior field `(root, off)` is `sameInteriorField` as another when
/// both roots match and both offsets are statically known and equal. When
/// provenance/offset is unknown we return false (permissive) — see the note
/// at the top of this file.
fn same_field(a: &View, b: &View) -> bool {
    a.0 == b.0 && a.1 == b.1
}

/// batch 2: detect `&mut` of an interior field while a *different* register
/// holds a live borrow on the same (root, offset) field. This is the sars
/// counterpart of sci's `hasOverlappingInteriorBorrow` guard inside
/// `armBorrow`. Same-register shared+mut conflicts are already reported by
/// the main dataflow `add_borrow`, so they are deliberately skipped here to
/// avoid duplicate diagnostics.
///
/// Linear intra-function pre-pass; no backend/merge changes.
pub fn check_overlapping_borrows<S: RegStateBackend>(
    function: &sa_ir::Function,
    ctx: &mut RuleCtx,
) {
    // Interior field identity for every pointer-carrying register. Built by
    // `ptr_add` from an allocation root (or an already-known field), with
    // the static byte offset accumulated along the chain (sci
    // `setInteriorOffsetFromBase`).
    let mut fields: HashMap<String, View> = HashMap::new();
    // Live borrows keyed by the *source* register (sci `locks[src] += 1`).
    let mut locks: HashMap<String, BorrowLock> = HashMap::new();
    // Borrow-view register -> its underlying source register (sci `origins`).
    let mut view_origin: HashMap<String, String> = HashMap::new();

    for ins in function.all_instructions() {
        match &ins.kind {
            InstructionKind::Alloc { .. } | InstructionKind::StackAlloc { .. } => {
                // An allocation root has field identity (root=itself, off=0),
                // but we only need to track it once it is derived by ptr_add.
            }
            InstructionKind::PtrAdd { base, offset } => {
                if let (Some(dest), Operand::Reg(base_reg)) = (ins.dest.as_deref(), base) {
                    let o = int_const(offset).unwrap_or(0);
                    if let Some((root, off)) = fields.get(base_reg).cloned() {
                        fields.insert(dest.to_string(), (root, off.saturating_add(o)));
                    } else {
                        fields.insert(dest.to_string(), (base_reg.clone(), o));
                    }
                }
            }
            InstructionKind::Move { src } => {
                if let (Some(dest), Operand::Reg(src_reg)) = (ins.dest.as_deref(), src) {
                    if let Some(f) = fields.get(src_reg).cloned() {
                        fields.insert(dest.to_string(), f);
                    }
                }
            }
            InstructionKind::Borrow { src, mutable } | InstructionKind::AssumeBorrow { src, mutable } => {
                let source = match src {
                    Operand::Reg(name) => name.clone(),
                    _ => continue,
                };
                if *mutable {
                    if let Some(field) = fields.get(&source).cloned() {
                        let overlapping = locks.iter().any(|(held_src, (_held_mut, held_field))| {
                            held_src != &source && same_field(&field, held_field)
                        });
                        if overlapping {
                            ctx.report(
                                Trap::ReadWriteConflict,
                                "cannot borrow mut while overlapping field borrows are active",
                                &ins.loc,
                                Some(&source),
                                None,
                                false,
                            );
                        }
                    }
                }
                // Record this borrow on the source register. A mutable borrow
                // replaces any shared locks on that same source; a shared
                // borrow adds another reader (mirrors setBorrowState/locks).
                if *mutable {
                    locks.insert(source.clone(), (true, fields.get(&source).cloned().unwrap_or_default()));
                } else if let Some((m, f)) = locks.get(&source).cloned() {
                    if !m {
                        locks.insert(source.clone(), (false, f));
                    }
                } else {
                    locks.insert(source.clone(), (false, fields.get(&source).cloned().unwrap_or_default()));
                }
                if let Some(dest) = ins.dest.as_deref() {
                    view_origin.insert(dest.to_string(), source);
                }
            }
            InstructionKind::Release { reg } => {
                if let Some(source) = view_origin.remove(reg) {
                    locks.remove(&source);
                }
            }
            _ => {}
        }
    }
}

/// E4: reject a `store` through a borrow view whose origin has more than one
/// live shared borrow (sci `writeCheck`, collect_metadata.zig:500-509). A
/// single shared borrow still grants a "materialize" write that upgrades the
/// origin to locked_mut (sci armStore:92-98); two or more readers make that
/// write a `ReadWriteConflict` ("cannot write through a shared borrow").
///
/// Linear intra-function pre-pass; no backend/merge changes.
pub fn check_shared_borrow_writes<S: RegStateBackend>(
    function: &sa_ir::Function,
    ctx: &mut RuleCtx,
) {
    let mut view_origin: HashMap<String, String> = HashMap::new();
    // Number of live *shared* borrows on each origin register.
    let mut shared_count: HashMap<String, usize> = HashMap::new();
    // Origins currently under an exclusive (&mut) borrow.
    let mut mut_origins: std::collections::HashSet<String> = std::collections::HashSet::new();

    for ins in function.all_instructions() {
        match &ins.kind {
            InstructionKind::Borrow { src, mutable } | InstructionKind::AssumeBorrow { src, mutable } => {
                let source = match src {
                    Operand::Reg(name) => name.clone(),
                    _ => continue,
                };
                if *mutable {
                    mut_origins.insert(source.clone());
                } else {
                    *shared_count.entry(source.clone()).or_insert(0) += 1;
                }
                if let Some(dest) = ins.dest.as_deref() {
                    view_origin.insert(dest.to_string(), source);
                }
            }
            InstructionKind::Release { reg } => {
                if let Some(source) = view_origin.remove(reg) {
                    if mut_origins.remove(&source) {
                        // exclusive borrow released
                    } else if let Some(c) = shared_count.get_mut(&source) {
                        *c = c.saturating_sub(1);
                    }
                }
            }
            InstructionKind::Store { addr, .. } => {
                if let Operand::Reg(base) = &addr.base {
                    if let Some(origin) = view_origin.get(base) {
                        let readers = shared_count.get(origin).copied().unwrap_or(0);
                        if readers > 1 {
                            ctx.report(
                                Trap::ReadWriteConflict,
                                "cannot write through a shared borrow",
                                &ins.loc,
                                Some(base),
                                None,
                                false,
                            );
                        }
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

    // ---- batch 2: overlapping interior-field borrow ----

    fn run_overlap(kinds: Vec<Instruction>) -> Vec<sa_ir::Diagnostic> {
        let mut f = Function::new(Signature::new("main"), loc(1));
        let mut b = Block::new("L_entry", loc(1));
        b.instrs = kinds;
        f.blocks.push(b);
        let mut program = sa_ir::Program::new();
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
        crate::bounds::check_overlapping_borrows::<crate::state::FnState>(
            &program.functions[0],
            &mut ctx,
        );
        sink.errors
    }

    fn alloc16(line: u32) -> Instruction {
        let mut a = ins(InstructionKind::Alloc { size: Operand::Int(16) }, line);
        a.dest = Some("buf".into());
        a
    }
    fn ptradd(dest: &str, base: &str, off: i128, line: u32) -> Instruction {
        let mut p = ins(
            InstructionKind::PtrAdd {
                base: Operand::Reg(base.into()),
                offset: Operand::Int(off),
            },
            line,
        );
        p.dest = Some(dest.into());
        p
    }
    fn borrow(dest: &str, src: &str, mutable: bool, line: u32) -> Instruction {
        let mut b = ins(
            InstructionKind::Borrow {
                src: Operand::Reg(src.into()),
                mutable,
            },
            line,
        );
        b.dest = Some(dest.into());
        b
    }

    #[test]
    fn overlapping_field_mut_borrow_is_rejected() {
        // buf=alloc16; f0=ptr_add buf,0; f1=ptr_add buf,0;
        // r0=&f0; r1=&mut f1 -> ReadWriteConflict (E1011)
        let errs = run_overlap(vec![
            alloc16(2),
            ptradd("f0", "buf", 0, 3),
            ptradd("f1", "buf", 0, 4),
            borrow("r0", "f0", false, 5),
            borrow("r1", "f1", true, 6),
        ]);
        assert!(errs.iter().any(|d| d.trap == Trap::ReadWriteConflict
            && d.message.contains("overlapping field borrows are active")));
    }

    #[test]
    fn distinct_fields_mut_borrow_is_allowed() {
        // Two different offsets are different fields -> no conflict.
        let errs = run_overlap(vec![
            alloc16(2),
            ptradd("f0", "buf", 0, 3),
            ptradd("f1", "buf", 4, 4),
            borrow("r0", "f0", false, 5),
            borrow("r1", "f1", true, 6),
        ]);
        assert!(errs.is_empty(), "distinct offsets must not conflict: {:?}", errs);
    }

    #[test]
    fn same_register_conflict_not_duplicated_by_prepass() {
        // Same-register shared+mut is the dataflow backend's job; the
        // pre-pass must skip it to avoid a duplicate diagnostic.
        let errs = run_overlap(vec![
            alloc16(2),
            ptradd("f0", "buf", 0, 3),
            borrow("r0", "f0", false, 4),
            borrow("r1", "f0", true, 5),
        ]);
        assert!(errs.is_empty(), "same-register conflict must be left to backend: {:?}", errs);
    }

    #[test]
    fn released_borrow_does_not_conflict() {
        let mut rel = ins(InstructionKind::Release { reg: "r0".into() }, 6);
        let errs = run_overlap(vec![
            alloc16(2),
            ptradd("f0", "buf", 0, 3),
            ptradd("f1", "buf", 0, 4),
            borrow("r0", "f0", false, 5),
            rel.clone(),
            borrow("r1", "f1", true, 7),
        ]);
        let _ = &mut rel;
        assert!(errs.is_empty(), "released borrow must not conflict: {:?}", errs);
    }

    // ---- E4: write through shared borrow ----

    fn run_store(kinds: Vec<Instruction>) -> Vec<sa_ir::Diagnostic> {
        let mut f = Function::new(Signature::new("main"), loc(1));
        let mut b = Block::new("L_entry", loc(1));
        b.instrs = kinds;
        f.blocks.push(b);
        let mut program = sa_ir::Program::new();
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
        crate::bounds::check_shared_borrow_writes::<crate::state::FnState>(
            &program.functions[0],
            &mut ctx,
        );
        sink.errors
    }

    fn store_view(base: &str, line: u32) -> Instruction {
        ins(
            InstructionKind::Store {
                addr: sa_ir::Address::new(Operand::Reg(base.into()), 0),
                value: Operand::Int(9),
            },
            line,
        )
    }

    #[test]
    fn write_through_two_shared_borrows_is_rejected() {
        let errs = run_store(vec![
            alloc16(2),
            borrow("v0", "buf", false, 3),
            borrow("v1", "buf", false, 4),
            store_view("v0", 5),
        ]);
        assert!(errs.iter().any(|d| d.trap == Trap::ReadWriteConflict
            && d.message.contains("cannot write through a shared borrow")));
    }

    #[test]
    fn write_through_single_shared_borrow_is_allowed() {
        // sci upgrades a lone reader to locked_mut on the store (armStore:92-98).
        let errs = run_store(vec![
            alloc16(2),
            borrow("v0", "buf", false, 3),
            store_view("v0", 4),
        ]);
        assert!(errs.is_empty(), "single shared borrow may materialize a write: {:?}", errs);
    }

    #[test]
    fn write_after_one_shared_released_is_allowed() {
        let rel = ins(InstructionKind::Release { reg: "v1".into() }, 5);
        let errs = run_store(vec![
            alloc16(2),
            borrow("v0", "buf", false, 3),
            borrow("v1", "buf", false, 4),
            rel,
            store_view("v0", 6),
        ]);
        assert!(errs.is_empty(), "released reader must allow write: {:?}", errs);
    }
}
