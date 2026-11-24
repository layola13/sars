use crate::buf::Buf;
use crate::ident::{const_name, js_float, js_func_name, js_ident, js_int, js_string};
use sa_ir::{
    AtomicRmwOp, Diagnostic, Instruction, InstructionKind, OpKind, Operand, PrimType, Program,
    Trap,
};
use std::collections::{BTreeSet, HashMap, HashSet};

const ZERO: Operand = Operand::Int(0);

pub struct ByteConsts {
    items: Vec<(String, Vec<u8>)>,
}

impl ByteConsts {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn from_program(program: &Program) -> Self {
        let mut out = Self::new();
        for f in &program.functions {
            for ins in f.all_instructions() {
                for op in ins.reads() {
                    out.note(op);
                }
            }
        }
        out
    }

    fn note(&mut self, op: &Operand) {
        match op {
            Operand::Bytes(b) => {
                self.symbol_for(b);
            }
            Operand::Utf8(s) => {
                self.symbol_for(s.as_bytes());
            }
            _ => {}
        }
    }

    pub fn symbol_for(&mut self, bytes: &[u8]) -> String {
        for (name, data) in &self.items {
            if data == bytes {
                return name.clone();
            }
        }
        let name = format!("C_B{}", self.items.len());
        self.items.push((name.clone(), bytes.to_vec()));
        name
    }

    pub fn items(&self) -> &[(String, Vec<u8>)] {
        &self.items
    }
}

pub struct FuncCtx<'a> {
    pub func: &'a str,
    pub regs: HashMap<String, String>,
    pub labels: HashMap<String, usize>,
    pub consts: HashSet<String>,
    pub functions: HashSet<String>,
    pub bytes: &'a mut ByteConsts,
    pub fallible: bool,
    pub debug: bool,
}

impl<'a> FuncCtx<'a> {
    pub fn reg_name(name: &str) -> String {
        format!("r_{}", js_ident(name))
    }

    pub fn label_id(&self, name: &str) -> Result<usize, String> {
        if let Some(id) = self.labels.get(name) {
            return Ok(*id);
        }
        let trimmed = name.trim_start_matches('@');
        if let Some(id) = self.labels.get(trimmed) {
            return Ok(*id);
        }
        Err(format!("unknown label: {name}"))
    }

    pub fn expr(&mut self, op: &Operand) -> Result<String, String> {
        match op {
            Operand::Reg(name) => {
                if self.consts.contains(name) {
                    Ok(const_name(name))
                } else if self.labels.contains_key(name) {
                    Ok(self.label_id(name)?.to_string())
                } else {
                    Ok(self.regs.get(name).cloned().unwrap_or_else(|| Self::reg_name(name)))
                }
            }
            Operand::Label(name) => Ok(self.label_id(name)?.to_string()),
            Operand::Int(v) => Ok(js_int(*v)),
            Operand::Float(v) => Ok(js_float(*v)),
            Operand::Bytes(b) => Ok(self.bytes.symbol_for(b)),
            Operand::Utf8(s) => Ok(self.bytes.symbol_for(s.as_bytes())),
            Operand::Symbol(name) => {
                if self.consts.contains(name) {
                    Ok(const_name(name))
                } else if self.functions.contains(name) {
                    Ok(format!("SA_FUNCS_BY_NAME[{}]", js_string(name)))
                } else {
                    Ok("0".to_string())
                }
            }
            Operand::Null | Operand::Undef => Ok("0".to_string()),
        }
    }
}

fn diagnostic(trap: Trap, msg: impl Into<String>, ins: &Instruction, func: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::new(trap, msg, ins.loc.clone()).in_function(func)]
}

pub fn build_ctx<'a>(
    program: &'a Program,
    f: &'a sa_ir::Function,
    bytes: &'a mut ByteConsts,
    debug: bool,
) -> FuncCtx<'a> {
    let mut regs = BTreeSet::new();
    for p in &f.sig.params {
        regs.insert(p.name.clone());
    }
    for r in &f.locals {
        regs.insert(r.clone());
    }
    for ins in f.all_instructions() {
        for d in ins.all_dests() {
            regs.insert(d.to_string());
        }
        for op in ins.reads() {
            if let Operand::Reg(name) = op {
                regs.insert(name.clone());
            }
        }
    }
    regs.insert("sa_tmp".to_string());

    let mut labels = HashMap::new();
    for (i, b) in f.blocks.iter().enumerate() {
        labels.insert(b.label.clone(), i);
        labels.insert(b.label.trim_start_matches('@').to_string(), i);
    }

    let consts = program.consts.iter().map(|c| c.name.clone()).collect();
    let functions = program.functions.iter().map(|x| x.sig.name.clone()).collect();
    FuncCtx {
        func: &f.sig.name,
        regs: regs.into_iter().map(|n| (n.clone(), FuncCtx::reg_name(&n))).collect(),
        labels,
        consts,
        functions,
        bytes,
        fallible: f.sig.return_fallible,
        debug,
    }
}

pub fn emit_function(
    f: &sa_ir::Function,
    buf: &mut Buf,
    ctx: &mut FuncCtx<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let jsname = js_func_name(&f.sig.name);
    let params: Vec<String> = f
        .sig
        .params
        .iter()
        .map(|p| FuncCtx::reg_name(&p.name))
        .collect();
    buf.push_indent(0, &format!("function {jsname}({}) {{", params.join(", ")));
    buf.push_indent(1, "let bb = 0;");
    for (_raw, js) in &ctx.regs {
        if params.iter().any(|p| p == js) {
            continue;
        }
        buf.push_indent(1, &format!("let {js} = 0;"));
    }
    buf.push_indent(1, "while (true) {");
    buf.push_indent(2, "switch (bb) {");

    for (i, block) in f.blocks.iter().enumerate() {
        buf.push_indent(3, &format!("case {i}: {{"));
        if ctx.debug {
            buf.push_indent(4, &format!("sa_trace({}, {i});", js_string(&f.sig.name)));
        }
        let mut terminated = false;
        for ins in &block.instrs {
            terminated = lower_instr(ins, ctx, buf, 4, i, f.blocks.len())?;
        }
        if !terminated {
            match f.blocks.get(i + 1) {
                Some(_) => buf.push_indent(4, &format!("bb = {}; break;", i + 1)),
                None => buf.push_indent(4, "sa_panic(1014, \"fallthrough end of function\");"),
            }
        }
        buf.push_indent(3, "}");
    }

    buf.push_indent(3, "default: sa_panic(2002, \"bad block\");");
    buf.push_indent(2, "}");
    buf.push_indent(1, "}");
    buf.push_indent(0, "}");
    Ok(())
}

fn dst(ins: &Instruction) -> String {
    match &ins.dest {
        Some(d) => FuncCtx::reg_name(d),
        None => "sa_tmp".to_string(),
    }
}

fn return_stmt(value: &Operand, ctx: &mut FuncCtx<'_>) -> Result<String, String> {
    let v = ctx.expr(value)?;
    if ctx.fallible {
        Ok(format!("return {{v: {v}, ok: true}};"))
    } else {
        Ok(format!("return {v};"))
    }
}

fn lower_instr(
    ins: &Instruction,
    ctx: &mut FuncCtx<'_>,
    buf: &mut Buf,
    indent: usize,
    block_id: usize,
    block_count: usize,
) -> Result<bool, Vec<Diagnostic>> {
    let func = ctx.func.to_string();
    let d = dst(ins);
    let map_err = |e: String| diagnostic(Trap::RuntimeError, e, ins, &func);
    let _ = (block_id, block_count);

    match &ins.kind {
        InstructionKind::Alloc { size } => {
            let v = ctx.expr(size).map_err(map_err)?;
            buf.push_indent(indent, &format!("{d} = sa_alloc({v});"));
        }
        InstructionKind::StackAlloc { size } => {
            let v = ctx.expr(size).map_err(map_err)?;
            buf.push_indent(indent, &format!("{d} = sa_stack_alloc({v});"));
        }
        InstructionKind::Load { addr } => {
            let a = ctx.expr(&addr.base).map_err(map_err)?;
            let off = addr.offset;
            let a = if off == 0 { a } else { format!("sa_ptr_add({a}, {off})") };
            let ty = ins.ty.unwrap_or(PrimType::I64);
            let code = js_string(ty_code(ty));
            buf.push_indent(indent, &format!("{d} = sa_load({a}, {code}, {});", ty.is_signed()));
        }
        InstructionKind::Store { addr, value } => {
            let a = ctx.expr(&addr.base).map_err(map_err)?;
            let off = addr.offset;
            let a = if off == 0 { a } else { format!("sa_ptr_add({a}, {off})") };
            let v = ctx.expr(value).map_err(map_err)?;
            let ty = ins.ty.unwrap_or(PrimType::I64);
            let code = js_string(ty_code(ty));
            buf.push_indent(indent, &format!("sa_store({a}, {v}, {code});"));
        }
        InstructionKind::Op { op, args } => {
            let e = op_expr(*op, args, ins.ty, ctx).map_err(map_err)?;
            buf.push_indent(indent, &format!("{d} = {e};"));
        }
        InstructionKind::Jmp { target } => {
            let id = ctx.label_id(target).map_err(map_err)?;
            buf.push_indent(indent, &format!("bb = {id}; break;"));
            return Ok(true);
        }
        InstructionKind::Br { cond, then_label, else_label } => {
            let c = ctx.expr(cond).map_err(map_err)?;
            let t = ctx.label_id(then_label).map_err(map_err)?;
            let e = ctx.label_id(else_label).map_err(map_err)?;
            buf.push_indent(indent, &format!("bb = ({c}) ? {t} : {e}; break;"));
            return Ok(true);
        }
        InstructionKind::BrNull { cond, target, else_target } => {
            let c = ctx.expr(cond).map_err(map_err)?;
            let id = ctx.label_id(target).map_err(map_err)?;
            if let Some(other) = else_target {
                let other_id = ctx.label_id(other).map_err(map_err)?;
                buf.push_indent(indent, &format!("bb = ({c}) ? {other_id} : {id}; break;"));
                return Ok(true);
            }
            buf.push_indent(indent, &format!("if (({c}) == 0) {{ bb = {id}; break; }}"));
        }
        InstructionKind::Call { target, args } => {
            let args = call_args(args, ctx)?;
            let call = format!("call({}, {args})", js_string(target));
            if ins.dest.is_some() {
                buf.push_indent(indent, &format!("{d} = {call};"));
            } else {
                buf.push_indent(indent, &format!("{call};"));
            }
        }
        InstructionKind::CallIndirect { target, args } => {
            let t = ctx.expr(target).map_err(map_err)?;
            let args = call_args(args, ctx)?;
            let call = format!("call({t}, {args})");
            if ins.dest.is_some() {
                buf.push_indent(indent, &format!("{d} = {call};"));
            } else {
                buf.push_indent(indent, &format!("{call};"));
            }
        }
        InstructionKind::Return { value } => {
            match value {
                Some(v) => buf.push_indent(indent, &return_stmt(v, ctx).map_err(map_err)?),
                None => {
                    if ctx.fallible {
                        buf.push_indent(indent, "return {v: 0, ok: true};");
                    } else {
                        buf.push_indent(indent, "return;");
                    }
                }
            }
            return Ok(true);
        }
        InstructionKind::EarlyReturn { value } => {
            match value {
                Some(v) => buf.push_indent(indent, &return_stmt(v, ctx).map_err(map_err)?),
                None => {
                    if ctx.fallible {
                        buf.push_indent(indent, "return {v: 0, ok: true};");
                    } else {
                        buf.push_indent(indent, "return;");
                    }
                }
            }
            return Ok(true);
        }
        InstructionKind::Take { addr } => {
            let a = ctx.expr(&addr.base).map_err(map_err)?;
            let off = addr.offset;
            let a = if off == 0 { a } else { format!("sa_ptr_add({a}, {off})") };
            let ty = ins.ty.unwrap_or(PrimType::I64);
            let code = js_string(ty_code(ty));
            buf.push_indent(indent, &format!("{d} = sa_load({a}, {code}, {});", ty.is_signed()));
        }
        InstructionKind::Release { .. } => {}
        InstructionKind::Assign { src } | InstructionKind::Move { src } => {
            let v = ctx.expr(src).map_err(map_err)?;
            buf.push_indent(indent, &format!("{d} = {v};"));
            if matches!(ins.kind, InstructionKind::Move { .. }) {
                if let Operand::Reg(name) = src {
                    let s = FuncCtx::reg_name(name);
                    if s != d {
                        buf.push_indent(indent, &format!("{s} = 0;"));
                    }
                }
            }
        }
        InstructionKind::PtrAdd { base, offset } => {
            let b = ctx.expr(base).map_err(map_err)?;
            let o = ctx.expr(offset).map_err(map_err)?;
            buf.push_indent(indent, &format!("{d} = sa_ptr_add({b}, {o});"));
        }
        InstructionKind::Borrow { src, .. }
        | InstructionKind::RawCast { src }
        | InstructionKind::AssumeSafe { src }
        | InstructionKind::AssumeBorrow { src, .. } => {
            let v = ctx.expr(src).map_err(map_err)?;
            buf.push_indent(indent, &format!("{d} = {v};"));
        }
        InstructionKind::AtomicLoad { addr, .. } => {
            let a = ctx.expr(&addr.base).map_err(map_err)?;
            let off = addr.offset;
            let a = if off == 0 { a } else { format!("sa_ptr_add({a}, {off})") };
            buf.push_indent(indent, &format!("{d} = Atomics.load(I32, sa_atomic_index({a}));"));
        }
        InstructionKind::AtomicStore { addr, value, .. } => {
            let a = ctx.expr(&addr.base).map_err(map_err)?;
            let off = addr.offset;
            let a = if off == 0 { a } else { format!("sa_ptr_add({a}, {off})") };
            let v = ctx.expr(value).map_err(map_err)?;
            buf.push_indent(indent, &format!("Atomics.store(I32, sa_atomic_index({a}), sa_num({v}));"));
        }
        InstructionKind::Cmpxchg { addr, expected, replacement, .. } => {
            let a = ctx.expr(&addr.base).map_err(map_err)?;
            let off = addr.offset;
            let a = if off == 0 { a } else { format!("sa_ptr_add({a}, {off})") };
            let e = ctx.expr(expected).map_err(map_err)?;
            let r = ctx.expr(replacement).map_err(map_err)?;
            buf.push_indent(indent, &format!("{d} = Atomics.compareExchange(I32, sa_atomic_index({a}), sa_num({e}), sa_num({r}));"));
            for extra in &ins.extra_dests {
                let name = FuncCtx::reg_name(extra);
                buf.push_indent(indent, &format!("{name} = ({d} === sa_num({e}) ? 1 : 0);"));
            }
        }
        InstructionKind::AtomicRmw { op, addr, value, .. } => {
            let a = ctx.expr(&addr.base).map_err(map_err)?;
            let off = addr.offset;
            let a = if off == 0 { a } else { format!("sa_ptr_add({a}, {off})") };
            let v = ctx.expr(value).map_err(map_err)?;
            let o = atomic_op_name(*op);
            buf.push_indent(indent, &format!("{d} = sa_atomic_rmw({}, {a}, {v});", js_string(o)));
        }
        InstructionKind::Fence { .. } => {}
        InstructionKind::Try { src } => {
            let v = ctx.expr(src).map_err(map_err)?;
            buf.push_indent(indent, &format!("{d} = sa_unwrap({v});"));
        }
        InstructionKind::Panic { message, args } => {
            let mut parts = Vec::new();
            let base = message.clone().unwrap_or_else(|| "panic".to_string());
            parts.push(js_string(&base));
            for a in args {
                parts.push(ctx.expr(a).map_err(map_err)?);
            }
            let msg = if parts.len() == 1 {
                parts.remove(0)
            } else {
                format!("{} + \" \" + [{}].join(\",\")", parts[0], parts[1..].join(", "))
            };
            buf.push_indent(indent, &format!("sa_panic(2999, {msg});"));
            return Ok(true);
        }
        InstructionKind::VaStart => {
            buf.push_indent(indent, "SA_VA = SA_VARARGS.slice();");
        }
        InstructionKind::VaArg { src } => {
            let s = ctx.expr(src).map_err(map_err)?;
            buf.push_indent(indent, &format!("{d} = SA_VA.length ? SA_VA.shift() : ({s});"));
        }
        InstructionKind::VaEnd => {
            buf.push_indent(indent, "SA_VA = [];");
        }
        InstructionKind::Native { .. } => {
            return Err(diagnostic(
                Trap::UnsupportedInstruction,
                "native instruction is not supported by the JS backend",
                ins,
                &func,
            ));
        }
        InstructionKind::Nop => {}
    }
    Ok(false)
}

fn call_args(args: &[sa_ir::CallArg], ctx: &mut FuncCtx<'_>) -> Result<String, Vec<Diagnostic>> {
    let mut out = Vec::new();
    for a in args {
        out.push(ctx.expr(&a.value).map_err(|e| {
            vec![Diagnostic::new(Trap::RuntimeError, e, sa_ir::SourceLoc::unknown())]
        })?);
    }
    Ok(format!("[{}]", out.join(", ")))
}

fn arg(args: &[Operand], i: usize) -> &Operand {
    args.get(i).unwrap_or(&ZERO)
}

fn op_expr(op: OpKind, args: &[Operand], ty: Option<PrimType>, ctx: &mut FuncCtx<'_>) -> Result<String, String> {
    let a = ctx.expr(arg(args, 0))?;
    let b = ctx.expr(arg(args, 1))?;
    let c = ctx.expr(arg(args, 2))?;
    let unary = |name: &str, x: &str| format!("{name}({x})");
    let binary = |name: &str, x: &str, y: &str| format!("{name}({x}, {y})");
    Ok(match op {
        OpKind::Add => binary("sa_add", &a, &b),
        OpKind::Sub => binary("sa_sub", &a, &b),
        OpKind::Mul => binary("sa_mul", &a, &b),
        OpKind::SDiv => binary("sa_sdiv", &a, &b),
        OpKind::UDiv => binary("sa_udiv", &a, &b),
        OpKind::SRem => binary("sa_srem", &a, &b),
        OpKind::URem => binary("sa_urem", &a, &b),
        OpKind::Neg => unary("sa_neg", &a),
        OpKind::And => binary("sa_band", &a, &b),
        OpKind::Or => binary("sa_bor", &a, &b),
        OpKind::Xor => binary("sa_bxor", &a, &b),
        OpKind::Shl => binary("sa_shl", &a, &b),
        OpKind::LShr => binary("sa_lshr", &a, &b),
        OpKind::AShr => binary("sa_ashr", &a, &b),
        OpKind::Not => unary("sa_bnot", &a),
        OpKind::Eq => binary("sa_eq", &a, &b),
        OpKind::Ne => binary("sa_ne", &a, &b),
        OpKind::Slt => binary("sa_slt", &a, &b),
        OpKind::Sle => binary("sa_sle", &a, &b),
        OpKind::Sgt => binary("sa_sgt", &a, &b),
        OpKind::Sge => binary("sa_sge", &a, &b),
        OpKind::Ult => binary("sa_ult", &a, &b),
        OpKind::Ule => binary("sa_ule", &a, &b),
        OpKind::Ugt => binary("sa_ugt", &a, &b),
        OpKind::Uge => binary("sa_uge", &a, &b),
        OpKind::Div => binary("sa_div", &a, &b),
        OpKind::Rem => binary("sa_rem", &a, &b),
        OpKind::Gt => binary("sa_gt", &a, &b),
        OpKind::Lt => binary("sa_lt", &a, &b),
        OpKind::Shr => binary("sa_shr", &a, &b),
        OpKind::FAdd => binary("sa_fadd", &a, &b),
        OpKind::FSub => binary("sa_fsub", &a, &b),
        OpKind::FMul => binary("sa_fmul", &a, &b),
        OpKind::FDiv => binary("sa_fdiv", &a, &b),
        OpKind::FNeg => unary("sa_fneg", &a),
        OpKind::FcmpEq => binary("sa_fcmp_eq", &a, &b),
        OpKind::FcmpNe => binary("sa_fcmp_ne", &a, &b),
        OpKind::FcmpLt => binary("sa_fcmp_lt", &a, &b),
        OpKind::FcmpLe => binary("sa_fcmp_le", &a, &b),
        OpKind::FcmpGt => binary("sa_fcmp_gt", &a, &b),
        OpKind::FcmpGe => binary("sa_fcmp_ge", &a, &b),
        OpKind::Trunc => cvt("trunc", &a, ty),
        OpKind::Zext => cvt("zext", &a, ty),
        OpKind::Sext => cvt("sext", &a, ty),
        OpKind::Fptosi => cvt("fptosi", &a, ty),
        OpKind::Sitofp => cvt("sitofp", &a, ty),
        OpKind::Uitofp => cvt("uitofp", &a, ty),
        OpKind::Fptrunc => cvt("fptrunc", &a, ty),
        OpKind::Fpext => cvt("fpext", &a, ty),
        OpKind::Bitcast => cvt("bitcast", &a, ty),
        OpKind::AddV128 => v128("add", &a, &b),
        OpKind::SubV128 => v128("sub", &a, &b),
        OpKind::MulV128 => v128("mul", &a, &b),
        OpKind::ShuffleV128 => format!("sa_shuffle_v128({a}, {b}, {c})"),
        OpKind::ExtractLane => format!("sa_extract_lane({a}, {b})"),
        OpKind::InsertLane => format!("sa_insert_lane({a}, {b}, {c})"),
    })
}

fn cvt(kind: &str, x: &str, ty: Option<PrimType>) -> String {
    let ty = ty.unwrap_or(PrimType::I64);
    format!(
        "sa_cvt({}, {x}, {}, {}, {})",
        js_string(kind),
        ty.bits(),
        ty.is_signed(),
        ty.is_float()
    )
}

fn v128(op: &str, a: &str, b: &str) -> String {
    format!("sa_v128({}, {a}, {b})", js_string(op))
}

fn ty_code(ty: PrimType) -> &'static str {
    match ty {
        PrimType::Void => "i32",
        PrimType::I1 => "i1",
        PrimType::I8 => "i8",
        PrimType::I16 => "i16",
        PrimType::I32 => "i32",
        PrimType::I64 => "i64",
        PrimType::U8 => "u8",
        PrimType::U16 => "u16",
        PrimType::U32 => "u32",
        PrimType::U64 => "u64",
        PrimType::F32 => "f32",
        PrimType::F64 => "f64",
        PrimType::Ptr => "i64",
        PrimType::BlobHandle => "u64",
        PrimType::V128 => "v128",
    }
}

fn atomic_op_name(op: AtomicRmwOp) -> &'static str {
    match op {
        AtomicRmwOp::Add => "add",
        AtomicRmwOp::Sub => "sub",
        AtomicRmwOp::And => "and",
        AtomicRmwOp::Or => "or",
        AtomicRmwOp::Xor => "xor",
        AtomicRmwOp::Xchg => "xchg",
        AtomicRmwOp::Umin => "umin",
        AtomicRmwOp::Umax => "umax",
        AtomicRmwOp::Smin => "smin",
        AtomicRmwOp::Smax => "smax",
    }
}
