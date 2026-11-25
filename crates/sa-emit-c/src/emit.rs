use crate::runtime;
use sa_ir::{
    ConstValue, Diagnostic, Function, Instruction, InstructionKind, OpKind, Operand, PrimType,
    Program, Signature, SourceLoc, Trap,
};
use std::collections::HashMap;

fn diag(trap: Trap, message: impl Into<String>, loc: &SourceLoc, func: &str) -> Diagnostic {
    Diagnostic::new(trap, message, loc.clone()).in_function(func)
}

fn ident(raw: &str) -> String {
    let mut out = String::new();
    for (i, c) in raw.chars().enumerate() {
        if c.is_ascii_alphanumeric() || c == '_' {
            if i == 0 && c.is_ascii_digit() {
                out.push('_');
            }
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

fn c_type(ty: PrimType) -> &'static str {
    match ty {
        PrimType::Void => "void",
        PrimType::I1 => "int",
        PrimType::I8 => "int8_t",
        PrimType::U8 => "uint8_t",
        PrimType::I16 => "int16_t",
        PrimType::U16 => "uint16_t",
        PrimType::I32 => "int32_t",
        PrimType::U32 => "uint32_t",
        PrimType::I64 => "int64_t",
        PrimType::U64 => "uint64_t",
        PrimType::F32 => "float",
        PrimType::F64 => "double",
        PrimType::Ptr | PrimType::BlobHandle => "uint8_t *",
        PrimType::V128 => "unsigned __int128",
    }
}

fn deref_expr(ty: PrimType, addr: &str) -> String {
    format!("*({} *)({addr})", c_type(ty))
}

fn is_native_shim(name: &str) -> bool {
    matches!(name, "sa_print_bytes" | "sa_panic")
}

fn unsigned_ty(ty: PrimType) -> PrimType {
    match ty {
        PrimType::I1 | PrimType::I8 => PrimType::U8,
        PrimType::I16 => PrimType::U16,
        PrimType::I32 => PrimType::U32,
        PrimType::I64 => PrimType::U64,
        other => other,
    }
}

fn signed_ty(ty: PrimType) -> PrimType {
    match ty {
        PrimType::U8 => PrimType::I8,
        PrimType::U16 => PrimType::I16,
        PrimType::U32 => PrimType::I32,
        PrimType::U64 => PrimType::I64,
        other => other,
    }
}

struct Names {
    funcs: HashMap<String, String>,
    consts: HashMap<String, String>,
    anon: HashMap<Vec<u8>, String>,
    anon_out: Vec<(String, Vec<u8>)>,
    anon_seq: usize,
}

impl Names {
    fn new(program: &Program) -> Self {
        let mut funcs = HashMap::new();
        for (i, f) in program.functions.iter().enumerate() {
            funcs.insert(f.sig.name.clone(), format!("sars_f{}_{}", i, ident(&f.sig.name)));
        }
        for (i, e) in program.externs.iter().enumerate() {
            funcs.insert(
                e.name.clone(),
                if is_native_shim(&e.name) {
                    e.name.clone()
                } else {
                    format!("sa_ext{}_{}", i, ident(&e.name))
                },
            );
        }
        let mut consts = HashMap::new();
        for (i, c) in program.consts.iter().enumerate() {
            consts.insert(c.name.clone(), format!("sa_c{}_{}", i, ident(&c.name)));
        }
        Self {
            funcs,
            consts,
            anon: HashMap::new(),
            anon_out: Vec::new(),
            anon_seq: 0,
        }
    }

    fn intern(&mut self, bytes: Vec<u8>) -> String {
        if let Some(name) = self.anon.get(&bytes) {
            return name.clone();
        }
        let name = format!("sa_k{}", self.anon_seq);
        self.anon_seq += 1;
        self.anon.insert(bytes.clone(), name.clone());
        self.anon_out.push((name.clone(), bytes));
        name
    }

    fn intern_operands(&mut self, program: &Program) {
        let instrs: Vec<&Instruction> = program
            .functions
            .iter()
            .flat_map(|f| f.all_instructions())
            .collect();
        for ins in instrs {
            for op in ins.reads() {
                match op {
                    Operand::Utf8(s) => {
                        self.intern(s.as_bytes().to_vec());
                    }
                    Operand::Bytes(b) => {
                        self.intern(b.clone());
                    }
                    Operand::Symbol(name) => {
                        if !self.consts.contains_key(name) {
                            if let Some(d) = program.find_def(name) {
                                self.intern(d.as_bytes().to_vec());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        for def in &program.defs {
            self.intern(def.value.as_bytes().to_vec());
        }
    }
}

struct FnCtx<'a> {
    func: &'a Function,
    program: &'a Program,
    names: &'a mut Names,
    regs: HashMap<String, String>,
    types: HashMap<String, PrimType>,
}

impl<'a> FnCtx<'a> {
    fn new(func: &'a Function, program: &'a Program, names: &'a mut Names) -> Self {
        let mut ctx = Self {
            func,
            program,
            names,
            regs: HashMap::new(),
            types: HashMap::new(),
        };
        for (i, p) in func.sig.params.iter().enumerate() {
            ctx.regs.insert(p.name.clone(), format!("a{}_{}", i, ident(&p.name)));
            ctx.types.insert(p.name.clone(), p.ty);
        }
        let instrs = func.all_instructions();
        for name in &func.locals {
            if ctx.regs.contains_key(name) {
                continue;
            }
            let hint = instrs
                .iter()
                .find(|i| i.dest.as_deref() == Some(name.as_str()) && i.ty.is_some())
                .and_then(|i| i.ty)
                .unwrap_or(PrimType::I64);
            let idx = ctx.regs.len();
            ctx.regs.insert(name.clone(), format!("r{}_{}", idx, ident(name)));
            ctx.types.insert(name.clone(), hint);
        }
        for ins in &instrs {
            for name in ins.all_dests() {
                if ctx.regs.contains_key(name) {
                    continue;
                }
                let ty = ctx.infer_ty(ins);
                let idx = ctx.regs.len();
                ctx.regs.insert(name.to_string(), format!("r{}_{}", idx, ident(name)));
                ctx.types.insert(name.to_string(), ty);
            }
        }
        for ins in &instrs {
            let Some(dest) = ins.dest.as_deref() else {
                continue;
            };
            let inferred = match &ins.kind {
                InstructionKind::Op { op, args } if ins.ty.is_none() => {
                    if op.is_comparison() {
                        Some(PrimType::I1)
                    } else {
                        ctx.type_of_operand(args.first())
                    }
                }
                InstructionKind::Call { target, .. } => ctx
                    .program
                    .function(target)
                    .map(|f| f.sig.return_value_type())
                    .or_else(|| {
                        ctx.program
                            .extern_sig(target)
                            .map(|s| s.return_value_type())
                    }),
                _ => ins.ty,
            };
            if let Some(ty) = inferred {
                ctx.types.insert(dest.to_string(), ty);
            }
        }
        ctx
    }

    fn infer_ty(&self, ins: &Instruction) -> PrimType {
        if let Some(ty) = ins.ty {
            return ty;
        }
        match &ins.kind {
            InstructionKind::Alloc { .. }
            | InstructionKind::StackAlloc { .. }
            | InstructionKind::PtrAdd { .. }
            | InstructionKind::Borrow { .. }
            | InstructionKind::RawCast { .. }
            | InstructionKind::AssumeSafe { .. }
            | InstructionKind::AssumeBorrow { .. } => PrimType::Ptr,
            InstructionKind::Assign { src } | InstructionKind::Move { src } => self
                .type_of_operand(Some(src))
                .unwrap_or(PrimType::I64),
            InstructionKind::Op { op, args } => {
                if op.is_comparison() {
                    PrimType::I1
                } else {
                    self.type_of_operand(args.first()).unwrap_or(PrimType::I64)
                }
            }
            InstructionKind::Call { target, .. } => self
                .program
                .function(target)
                .map(|f| f.sig.return_value_type())
                .or_else(|| {
                    self.program
                        .extern_sig(target)
                        .map(|s| s.return_value_type())
                })
                .unwrap_or(PrimType::I64),
            _ => PrimType::I64,
        }
    }

    fn type_of_operand(&self, op: Option<&Operand>) -> Option<PrimType> {
        match op? {
            Operand::Reg(name) => self.types.get(name).copied(),
            Operand::Int(_) => Some(PrimType::I64),
            Operand::Float(_) => Some(PrimType::F64),
            Operand::Utf8(_) | Operand::Bytes(_) | Operand::Symbol(_) | Operand::Null => {
                Some(PrimType::Ptr)
            }
            Operand::Label(_) | Operand::Undef => None,
        }
    }

    fn expr(&mut self, op: &Operand, hint: PrimType, diags: &mut Vec<Diagnostic>) -> String {
        match op {
            Operand::Reg(name) => {
                if let Some(c) = self.names.consts.get(name) {
                    return c.clone();
                }
                if let Some(r) = self.regs.get(name) {
                    return r.clone();
                }
                if let Some(d) = self.program.find_def(name) {
                    return self.names.intern(d.as_bytes().to_vec());
                }
                diags.push(diag(
                    Trap::UnknownRegister,
                    format!("unknown register {name}"),
                    &self.func.loc,
                    self.func.name(),
                ));
                "0".to_string()
            }
            Operand::Symbol(name) => {
                if let Some(c) = self.names.consts.get(name) {
                    return c.clone();
                }
                if let Some(d) = self.program.find_def(name) {
                    return self.names.intern(d.as_bytes().to_vec());
                }
                diags.push(diag(
                    Trap::UnknownRegister,
                    format!("unknown symbol @{name}"),
                    &self.func.loc,
                    self.func.name(),
                ));
                "0".to_string()
            }
            Operand::Int(v) => format!("({}){}", c_type(hint), v),
            Operand::Float(v) => format!("({}){}", c_type(hint), v),
            Operand::Utf8(s) => self.names.intern(s.as_bytes().to_vec()),
            Operand::Bytes(b) => self.names.intern(b.clone()),
            Operand::Null => {
                if hint.is_pointer() {
                    "(uint8_t *)0".to_string()
                } else {
                    "0".to_string()
                }
            }
            Operand::Undef => "0".to_string(),
            Operand::Label(label) => label.clone(),
        }
    }

    fn addr_expr(&mut self, addr: &sa_ir::Address, diags: &mut Vec<Diagnostic>) -> String {
        let base = self.expr(&addr.base, PrimType::Ptr, diags);
        if addr.offset == 0 {
            format!("({base})")
        } else {
            format!("((uint8_t *)({base}) + {})", addr.offset)
        }
    }
}

fn label_name(raw: &str) -> String {
    format!("L_{}", ident(raw))
}

fn call_signature<'a>(ctx: &FnCtx<'a>, target: &str) -> Option<&'a Signature> {
    ctx.program
        .function(target)
        .map(|f| &f.sig)
        .or_else(|| ctx.program.extern_sig(target))
}

fn op_expression(
    ctx: &mut FnCtx<'_>,
    op: OpKind,
    args: &[Operand],
    hint: PrimType,
    diags: &mut Vec<Diagnostic>,
) -> String {
    let src_ty = ctx.type_of_operand(args.first()).unwrap_or(hint);
    let arg_ty = if op.is_comparison() { src_ty } else { hint };
    let a = args
        .first()
        .map(|v| ctx.expr(v, arg_ty, diags))
        .unwrap_or_else(|| "0".to_string());
    let b = args
        .get(1)
        .map(|v| ctx.expr(v, arg_ty, diags))
        .unwrap_or_else(|| "0".to_string());
    if op.is_unary() {
        let cty = c_type(hint);
        return match op {
            OpKind::Neg | OpKind::FNeg => format!("-({cty})({a})"),
            OpKind::Not => format!("(~({cty})({a}))"),
            OpKind::Trunc => format!("({cty})({a})"),
            OpKind::Zext => format!("({cty})(({})({a}))", c_type(unsigned_ty(src_ty))),
            OpKind::Sext => format!("({cty})(({})({a}))", c_type(signed_ty(src_ty))),
            OpKind::Fptrunc | OpKind::Fpext => format!("({cty})({a})"),
            OpKind::Sitofp | OpKind::Uitofp => {
                let float_ty = if hint.is_float() { cty } else { "double" };
                format!("({float_ty})({a})")
            }
            OpKind::Fptosi => format!("({cty})({a})"),
            OpKind::Bitcast => {
                match (src_ty, hint) {
                    (PrimType::F64, PrimType::I64 | PrimType::U64) => {
                        format!("sa_bitcast_i64_f64({a})")
                    }
                    (PrimType::I64 | PrimType::U64, PrimType::F64) => {
                        format!("sa_bitcast_f64_i64({a})")
                    }
                    _ => {
                        diags.push(diag(
                            Trap::UnsupportedType,
                            "bitcast width is not supported by the C backend",
                            &ctx.func.loc,
                            ctx.func.name(),
                        ));
                        format!("({cty})({a})")
                    }
                }
            }
            _ => {
                diags.push(diag(
                    Trap::UnsupportedInstruction,
                    "unsupported unary op",
                    &ctx.func.loc,
                    ctx.func.name(),
                ));
                format!("({cty})({a})")
            }
        };
    }
    let cty = c_type(hint);
    let second_ty = ctx.type_of_operand(args.get(1));
    let mixed_sign = src_ty.is_signed()
        && second_ty
            .map(|t| t.is_integer() && !t.is_signed())
            .unwrap_or(false);
    let (cmp_a, cmp_b) = if op.is_comparison() {
        if src_ty.is_float() {
            (a.clone(), b.clone())
        } else if mixed_sign {
            let u = c_type(unsigned_ty(src_ty));
            (format!("({u})({a})"), format!("({u})({b})"))
        } else if src_ty.is_signed() {
            (format!("(int64_t)({a})"), format!("(int64_t)({b})"))
        } else {
            (format!("(uint64_t)({a})"), format!("(uint64_t)({b})"))
        }
    } else {
        (a.clone(), b.clone())
    };
    let shift_ty = c_type(unsigned_ty(src_ty));
    let signed_shift_ty = c_type(signed_ty(src_ty));
    let div_signed = matches!(op, OpKind::SDiv | OpKind::SRem)
        || (matches!(op, OpKind::Div | OpKind::Rem) && src_ty.is_signed());
    let div_ty = if div_signed { "int64_t" } else { "uint64_t" };
    let (div_a, div_b) = (
        format!("({div_ty})({a})"),
        format!("({div_ty})({b})"),
    );
    match op {
        OpKind::Add | OpKind::FAdd => format!("(({a}) + ({b}))"),
        OpKind::Sub | OpKind::FSub => format!("(({a}) - ({b}))"),
        OpKind::Mul | OpKind::FMul => format!("(({a}) * ({b}))"),
        OpKind::SDiv | OpKind::UDiv | OpKind::Div => {
            format!("(({div_b}) == 0 ? (sa_panic(), ({cty})0) : (({div_a}) / ({div_b})))")
        }
        OpKind::FDiv => format!("(({a}) / ({b}))"),
        OpKind::SRem | OpKind::URem | OpKind::Rem => {
            format!("(({div_b}) == 0 ? (sa_panic(), ({cty})0) : (({div_a}) % ({div_b})))")
        }
        OpKind::And => format!("(({a}) & ({b}))"),
        OpKind::Or => format!("(({a}) | ({b}))"),
        OpKind::Xor => format!("(({a}) ^ ({b}))"),
        OpKind::Shl => format!("(({shift_ty})({a}) << ({b}))"),
        OpKind::LShr => format!("(({shift_ty})({a}) >> ({b}))"),
        OpKind::AShr => format!("(({signed_shift_ty})({a}) >> ({b}))"),
        OpKind::Shr => {
            let ty = if src_ty.is_signed() { signed_shift_ty } else { shift_ty };
            format!("(({ty})({a}) >> ({b}))")
        }
        OpKind::Eq | OpKind::FcmpEq => format!("(({cmp_a}) == ({cmp_b}))"),
        OpKind::Ne | OpKind::FcmpNe => format!("(({cmp_a}) != ({cmp_b}))"),
        OpKind::Slt | OpKind::Lt | OpKind::FcmpLt => format!("(({cmp_a}) < ({cmp_b}))"),
        OpKind::Sle | OpKind::FcmpLe => format!("(({cmp_a}) <= ({cmp_b}))"),
        OpKind::Sgt | OpKind::Gt | OpKind::FcmpGt => format!("(({cmp_a}) > ({cmp_b}))"),
        OpKind::Sge | OpKind::FcmpGe => format!("(({cmp_a}) >= ({cmp_b}))"),
        OpKind::Ult => format!("((uint64_t)({a}) < (uint64_t)({b}))"),
        OpKind::Ule => format!("((uint64_t)({a}) <= (uint64_t)({b}))"),
        OpKind::Ugt => format!("((uint64_t)({a}) > (uint64_t)({b}))"),
        OpKind::Uge => format!("((uint64_t)({a}) >= (uint64_t)({b}))"),
        _ => {
            diags.push(diag(
                Trap::UnsupportedInstruction,
                "unsupported instruction in C backend",
                &ctx.func.loc,
                ctx.func.name(),
            ));
            "0".to_string()
        }
    }
}

fn unsupported(ctx: &FnCtx<'_>, diags: &mut Vec<Diagnostic>, loc: &SourceLoc, what: &str) {
    diags.push(diag(
        Trap::UnsupportedInstruction,
        format!("{what} is not implemented by the C backend (first increment)"),
        loc,
        ctx.func.name(),
    ));
}

fn emit_instruction(
    ctx: &mut FnCtx<'_>,
    out: &mut String,
    ins: &Instruction,
    diags: &mut Vec<Diagnostic>,
) {
    let dest = ins.dest.as_deref().map(|d| {
        ctx.regs
            .get(d)
            .cloned()
            .unwrap_or_else(|| format!("r_{}", ident(d)))
    });
    let assign = |out: &mut String, value: String| {
        if let Some(d) = &dest {
            out.push_str(&format!("  {d} = {value};\n"));
        }
    };
    match &ins.kind {
        InstructionKind::Nop | InstructionKind::Release { .. } => {}
        InstructionKind::Panic { .. } => out.push_str("  sa_panic();\n"),
        InstructionKind::Alloc { size } | InstructionKind::StackAlloc { size } => {
            let sz = ctx.expr(size, PrimType::U64, diags);
            assign(out, format!("sa_alloc((uint64_t)({sz}))"));
        }
        InstructionKind::Store { addr, value } => {
            let ty = ins
                .ty
                .unwrap_or_else(|| ctx.type_of_operand(Some(value)).unwrap_or(PrimType::I64));
            let p = ctx.addr_expr(addr, diags);
            let v = ctx.expr(value, ty, diags);
            out.push_str(&format!(
                "  {} = ({})({v});\n",
                deref_expr(ty, &p),
                c_type(ty)
            ));
        }
        InstructionKind::Load { addr } | InstructionKind::Take { addr } => {
            let ty = ins.ty.unwrap_or(PrimType::I64);
            let p = ctx.addr_expr(addr, diags);
            assign(out, deref_expr(ty, &p));
        }
        InstructionKind::Op { op, args } => {
            let hint = ins
                .dest
                .as_deref()
                .and_then(|d| ctx.types.get(d).copied())
                .or(ins.ty)
                .unwrap_or(PrimType::I64);
            let expr = op_expression(ctx, *op, args, hint, diags);
            assign(out, expr);
        }
        InstructionKind::Jmp { target } => {
            out.push_str(&format!("  goto {};\n", label_name(target)));
        }
        InstructionKind::Br {
            cond,
            then_label,
            else_label,
        } => {
            let c = ctx.expr(cond, PrimType::I1, diags);
            out.push_str(&format!(
                "  if (({c}) != 0) goto {}; else goto {};\n",
                label_name(then_label),
                label_name(else_label)
            ));
        }
        InstructionKind::BrNull {
            cond,
            target,
            else_target,
        } => {
            let c = ctx.expr(cond, PrimType::Ptr, diags);
            let yes = format!("  if (({c}) == (uint8_t *)0) goto {};\n", label_name(target));
            match else_target {
                Some(other) => {
                    out.push_str(&yes);
                    out.push_str(&format!("  else goto {};\n", label_name(other)));
                }
                None => out.push_str(&yes),
            }
        }
        InstructionKind::Call { target, args } => {
            let sig = match call_signature(ctx, target) {
                Some(s) => s.clone(),
                None => {
                    diags.push(diag(
                        Trap::UnknownRegister,
                        format!("call to unknown function {target}"),
                        &ins.loc,
                        ctx.func.name(),
                    ));
                    return;
                }
            };
            if sig.return_fallible {
                unsupported(ctx, diags, &ins.loc, "fallible call");
                return;
            }
            let sym = ctx
                .names
                .funcs
                .get(target)
                .cloned()
                .unwrap_or_else(|| ident(target));
            let mut rendered = Vec::new();
            for (arg, param) in args.iter().zip(sig.params.iter()) {
                rendered.push(ctx.expr(&arg.value, param.ty, diags));
            }
            let call = format!("{sym}({})", rendered.join(", "));
            if sig.return_value_type().is_void() || dest.is_none() {
                out.push_str(&format!("  {call};\n"));
            } else {
                assign(out, call);
            }
        }
        InstructionKind::CallIndirect { .. } => {
            unsupported(ctx, diags, &ins.loc, "call_indirect / vtable dispatch");
        }
        InstructionKind::Return { value } | InstructionKind::EarlyReturn { value } => {
            match value {
                Some(v) => {
                    let ret = ctx.expr(v, ctx.func.sig.return_value_type(), diags);
                    out.push_str(&format!(
                        "  return ({})({ret});\n",
                        c_type(ctx.func.sig.return_value_type())
                    ));
                }
                None => out.push_str("  return;\n"),
            }
        }
        InstructionKind::Assign { src }
        | InstructionKind::Move { src }
        | InstructionKind::Borrow { src, .. }
        | InstructionKind::RawCast { src }
        | InstructionKind::AssumeSafe { src }
        | InstructionKind::AssumeBorrow { src, .. } => {
            let ty = ins
                .dest
                .as_deref()
                .and_then(|d| ctx.types.get(d).copied())
                .unwrap_or_else(|| ctx.type_of_operand(Some(src)).unwrap_or(PrimType::I64));
            let v = ctx.expr(src, ty, diags);
            assign(out, v);
        }
        InstructionKind::PtrAdd { base, offset } => {
            let b = ctx.expr(base, PrimType::Ptr, diags);
            let o = ctx.expr(offset, PrimType::U64, diags);
            assign(out, format!("((uint8_t *)({b}) + ({o}))"));
        }
        InstructionKind::AtomicLoad { .. }
        | InstructionKind::AtomicStore { .. }
        | InstructionKind::Cmpxchg { .. }
        | InstructionKind::AtomicRmw { .. }
        | InstructionKind::Fence { .. } => {
            unsupported(ctx, diags, &ins.loc, "atomics / fences");
        }
        InstructionKind::Try { .. } => {
            unsupported(ctx, diags, &ins.loc, "fallible `?` unpacking");
        }
        InstructionKind::Native { .. }
        | InstructionKind::VaStart
        | InstructionKind::VaArg { .. }
        | InstructionKind::VaEnd => {
            unsupported(ctx, diags, &ins.loc, "variadic / native intrinsics");
        }
    }
}

fn emit_function(
    func: &Function,
    program: &Program,
    names: &mut Names,
    diags: &mut Vec<Diagnostic>,
) -> String {
    let sym = names
        .funcs
        .get(func.name())
        .cloned()
        .unwrap_or_else(|| ident(func.name()));
    let mut ctx = FnCtx::new(func, program, names);
    if func.sig.is_variadic {
        diags.push(diag(
            Trap::UnsupportedInstruction,
            "variadic functions are not supported by the C backend",
            &func.loc,
            func.name(),
        ));
    }
    if func.sig.return_fallible {
        diags.push(diag(
            Trap::UnsupportedInstruction,
            "fallible function signatures are not supported by the C backend (first increment)",
            &func.loc,
            func.name(),
        ));
    }
    for ty in func.sig.params.iter().map(|p| p.ty).chain([func.sig.return_ty]) {
        if ty == PrimType::V128 {
            diags.push(diag(
                Trap::UnsupportedType,
                "v128 is not supported by the C backend",
                &func.loc,
                func.name(),
            ));
        }
    }
    let ret = c_type(func.sig.return_value_type());
    let params: Vec<String> = func
        .sig
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{} a{}_{}", c_type(p.ty), i, ident(&p.name)))
        .collect();
    let mut out = format!("{ret} {sym}({}) {{\n", params.join(", "));
    let param_vars: Vec<String> = func
        .sig
        .params
        .iter()
        .enumerate()
        .filter_map(|(i, p)| ctx.regs.get(&p.name).cloned().or_else(|| Some(format!("a{}_{}", i, ident(&p.name)))))
        .collect();
    let mut decls: Vec<(String, PrimType)> = ctx
        .regs
        .iter()
        .filter(|(_, cvar)| !param_vars.contains(cvar))
        .map(|(name, cvar)| (cvar.clone(), ctx.types.get(name).copied().unwrap_or(PrimType::I64)))
        .collect();
    decls.sort_by(|a, b| a.0.cmp(&b.0));
    for (cvar, ty) in decls {
        out.push_str(&format!("  {} {};\n", c_type(ty), cvar));
    }
    for block in &func.blocks {
        out.push_str(&format!("{}:\n", label_name(&block.label)));
        for ins in &block.instrs {
            emit_instruction(&mut ctx, &mut out, ins, diags);
        }
    }
    out.push_str(&format!("  return ({})0;\n}}\n", c_type(func.sig.return_value_type())));
    out
}

fn static_bytes(out: &mut String, sym: &str, bytes: &[u8]) {
    let values: Vec<String> = bytes.iter().map(|b| b.to_string()).collect();
    out.push_str(&format!("static const unsigned char {sym}[] = {{"));
    if !values.is_empty() {
        out.push_str(&values.join(", "));
        out.push_str(", ");
    }
    out.push_str("0};\n");
}

fn const_data(program: &Program, names: &Names, diags: &mut Vec<Diagnostic>) -> String {
    let mut out = String::new();
    for c in &program.consts {
        let sym = match names.consts.get(&c.name) {
            Some(s) => s,
            None => continue,
        };
        let bytes = match &c.value {
            ConstValue::VTable(_) => {
                diags.push(Diagnostic::new(
                    Trap::VTableSignatureMismatch,
                    format!("vtable const {} is not supported by the C backend", c.name),
                    SourceLoc::unknown(),
                ));
                continue;
            }
            value => value.bytes().unwrap_or_default(),
        };
        static_bytes(&mut out, sym, &bytes);
    }
    out
}

fn prototypes(out: &mut String, program: &Program, names: &Names) {
    for f in &program.functions {
        let sym = names
            .funcs
            .get(f.name())
            .cloned()
            .unwrap_or_else(|| ident(f.name()));
        let params: Vec<String> = f.sig.params.iter().map(|p| c_type(p.ty).to_string()).collect();
        out.push_str(&format!(
            "{} {}({});\n",
            c_type(f.sig.return_value_type()),
            sym,
            params.join(", ")
        ));
    }
}

fn host_target(target: Option<&str>) -> bool {
    match target {
        None => true,
        Some(t) => {
            let t = t.to_ascii_lowercase();
            if t.contains("native") || t.contains("host") {
                return true;
            }
            t.contains(std::env::consts::ARCH)
        }
    }
}

pub fn emit(program: &Program, opts: &crate::COptions) -> Result<String, Vec<Diagnostic>> {
    let mut diags = Vec::new();
    if !host_target(opts.target.as_deref()) {
        diags.push(Diagnostic::new(
            Trap::UnsupportedInstruction,
            format!(
                "cross-target {:?} is not supported by the C backend (first increment); host only",
                opts.target
            ),
            SourceLoc::unknown(),
        ));
    }
    if !program.npm_binds.is_empty() {
        diags.push(Diagnostic::new(
            Trap::UnsupportedInstruction,
            "build-exe rejects #npm_bind programs (D22): npm bindings are JS-only",
            program.npm_binds[0].loc.clone(),
        ));
    }
    for e in &program.externs {
        if !is_native_shim(&e.name) {
            diags.push(Diagnostic::new(
                Trap::ImportResolutionFailed,
                format!(
                    "extern {} has no native runtime shim (C backend first increment)",
                    e.name
                ),
                SourceLoc::unknown(),
            ));
        }
    }
    if !diags.is_empty() {
        return Err(diags);
    }
    let mut names = Names::new(program);
    names.intern_operands(program);
    let data = const_data(program, &names, &mut diags);
    if !diags.is_empty() {
        return Err(diags);
    }
    let mut func_code = String::new();
    for f in &program.functions {
        func_code.push_str(&emit_function(f, program, &mut names, &mut diags));
        func_code.push('\n');
    }
    if !diags.is_empty() {
        return Err(diags);
    }
    let entry = program.function("main").or_else(|| program.entry_function());
    let entry_sym = entry
        .and_then(|f| names.funcs.get(f.name()).cloned())
        .unwrap_or_default();
    let entry_ret = entry
        .map(|f| f.sig.return_value_type())
        .unwrap_or(PrimType::I32);
    let mut out = String::new();
    out.push_str("/* generated by sars sa-emit-c - native C backend */\n");
    out.push_str(&runtime::runtime_source(opts.heap_size));
    out.push_str(&data);
    for (name, bytes) in &names.anon_out {
        static_bytes(&mut out, name, bytes);
    }
    prototypes(&mut out, program, &names);
    out.push_str(&func_code);
    out.push_str(&format!(
        "int main(void) {{ return ({})({}()); }}\n",
        c_type(entry_ret),
        entry_sym
    ));
    Ok(out)
}
