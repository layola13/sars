use crate::leb::{put_sleb, put_u8, put_uleb};
use crate::opcode::*;
use crate::sections::functype;
use crate::SA_FN_BASE;
use sa_ir::{
    Address, ConstValue, Diagnostic, Function, Instruction, InstructionKind, OpKind, Operand,
    PrimType, Program, Signature, SourceLoc, Trap,
};
use std::collections::HashMap;

pub struct FunctionCode {
    pub locals: Vec<(u32, u8)>,
    pub code: Vec<u8>,
}

struct Registers {
    map: HashMap<String, u32>,
    types: Vec<u8>,
    status: HashMap<String, u32>,
    scratch: [u32; 2],
    consts: HashMap<String, u32>,
    indirect_drops: HashMap<usize, u8>,
}

fn diag(trap: Trap, message: &str, loc: &SourceLoc, function: &str) -> Diagnostic {
    Diagnostic::new(trap, message, loc.clone()).in_function(function)
}

fn collect_registers(func: &Function, program: &Program, diags: &mut Vec<Diagnostic>) -> Registers {
    let mut map = HashMap::new();
    let mut types = Vec::new();
    for p in &func.sig.params {
        let ty = match storage_type(Some(p.ty)) {
            Some(t) => t,
            None => {
                diags.push(diag(
                    Trap::UnsupportedType,
                    "v128 parameters are not supported",
                    &func.loc,
                    func.name(),
                ));
                VALTYPE_I64
            }
        };
        map.insert(p.name.clone(), types.len() as u32);
        types.push(ty);
    }
    let instrs = func.all_instructions();
    for name in &func.locals {
        let hint = instrs
            .iter()
            .find(|i| i.dest.as_deref() == Some(name.as_str()) && i.ty.is_some())
            .and_then(|i| i.ty);
        let ty = storage_type(hint).unwrap_or(VALTYPE_I64);
        map.insert(name.clone(), types.len() as u32);
        types.push(ty);
    }
    for ins in &instrs {
        for name in ins.all_dests() {
            if !map.contains_key(name) {
                let ty = storage_type(ins.ty).unwrap_or(VALTYPE_I64);
                map.insert(name.to_string(), types.len() as u32);
                types.push(ty);
            }
        }
    }
    let scratch = [
        {
            let name = "__wa_scratch_0".to_string();
            map.insert(name, types.len() as u32);
            types.push(VALTYPE_I64);
            types.len() as u32 - 1
        },
        {
            let name = "__wa_scratch_1".to_string();
            map.insert(name, types.len() as u32);
            types.push(VALTYPE_I64);
            types.len() as u32 - 1
        },
    ];
    let consts: HashMap<String, u32> = crate::const_layout(program)
        .into_iter()
        .map(|segment| (segment.name, segment.addr))
        .collect();
    let mut status = HashMap::new();
    for name in map.keys() {
        let index = types.len() as u32;
        types.push(VALTYPE_I32);
        status.insert(name.clone(), index);
    }
    let mut indirect_drops = HashMap::new();
    let mut probe = Registers {
        map: map.clone(),
        types: types.clone(),
        status: status.clone(),
        scratch,
        consts: consts.clone(),
        indirect_drops: HashMap::new(),
    };
    for ins in &instrs {
        let Some(dest) = ins.dest.as_deref() else {
            continue;
        };
        let Some(&index) = map.get(dest) else {
            continue;
        };
        let result = match &ins.kind {
            InstructionKind::Op { op, args } => {
                let storage = match op {
                    OpKind::Sitofp | OpKind::Uitofp => VALTYPE_I64,
                    OpKind::Fptosi => VALTYPE_F64,
                    _ => operand_storage(args.first(), &probe, ins.ty),
                };
                Some(if op.is_comparison() {
                    VALTYPE_I64
                } else {
                    match op {
                        OpKind::Fptosi => VALTYPE_I64,
                        OpKind::Sitofp | OpKind::Uitofp => VALTYPE_F64,
                        _ => storage,
                    }
                })
            }
            InstructionKind::Call { target, .. } => call_sig(program, target)
                .filter(|sig| !sig.return_value_type().is_void())
                .map(|sig| storage_type(Some(sig.return_value_type())).unwrap_or(VALTYPE_I64)),
            _ => None,
        };
        if let Some(result) = result {
            probe.types[index as usize] = result;
            types[index as usize] = result;
        }
    }
    let signatures = vtable_signatures(program);
    for ins in &instrs {
        if let InstructionKind::CallIndirect { args, .. } = &ins.kind {
            let arg_storages: Vec<u8> = args
                .iter()
                .map(|a| operand_storage(Some(&a.value), &probe, None))
                .collect();
            let matches: Vec<&&Signature> = signatures
                .iter()
                .filter(|sig| {
                    sig.params.len() == arg_storages.len()
                        && sig
                            .params
                            .iter()
                            .zip(arg_storages.iter())
                            .all(|(p, s)| storage_type(Some(p.ty)).unwrap_or(VALTYPE_I64) == *s)
                })
                .collect();
            let result = matches
                .iter()
                .filter(|sig| !sig.return_value_type().is_void())
                .map(|sig| storage_type(Some(sig.return_value_type())).unwrap_or(VALTYPE_I64))
                .next();
            if let Some(result) = result {
                if let Some(name) = &ins.dest {
                    if let Some(&index) = map.get(name) {
                        types[index as usize] = result;
                        probe.types[index as usize] = result;
                    }
                } else {
                    indirect_drops.insert(*ins as *const Instruction as usize, result);
                }
            }
        }
    }
    Registers {
        map,
        types,
        status,
        scratch,
        consts,
        indirect_drops,
    }
}

fn vtable_signatures(program: &Program) -> Vec<&Signature> {
    let mut out = Vec::new();
    for constant in &program.consts {
        let ConstValue::VTable(entries) = &constant.value else {
            continue;
        };
        for entry in entries {
            if let Some(target) = entry.vtable_target() {
                if let Some(func) = program.functions.iter().find(|f| f.sig.name == target) {
                    out.push(&func.sig);
                }
            }
        }
    }
    out
}

fn local_get(out: &mut Vec<u8>, index: u32) {
    put_u8(out, LOCAL_GET);
    put_uleb(out, index as u64);
}

fn local_set(out: &mut Vec<u8>, index: u32) {
    put_u8(out, LOCAL_SET);
    put_uleb(out, index as u64);
}

fn store_or_drop(out: &mut Vec<u8>, ins: &Instruction, regs: &Registers) {
    if let Some(name) = &ins.dest {
        if let Some(&index) = regs.map.get(name) {
            local_set(out, index);
            return;
        }
    }
    put_u8(out, DROP);
}

fn emit_status(out: &mut Vec<u8>, op: &Operand, regs: &Registers) {
    if let Operand::Reg(name) = op {
        if let Some(&index) = regs.status.get(name) {
            local_get(out, index);
            return;
        }
    }
    i32_const(out, 0);
}

fn set_status(out: &mut Vec<u8>, dest: Option<&str>, regs: &Registers) {
    if let Some(name) = dest {
        if let Some(&index) = regs.status.get(name) {
            local_set(out, index);
            return;
        }
    }
    put_u8(out, DROP);
}

fn i64_const(out: &mut Vec<u8>, v: i64) {
    put_u8(out, I64_CONST);
    put_sleb(out, v);
}

fn i32_const(out: &mut Vec<u8>, v: i32) {
    put_u8(out, I32_CONST);
    put_sleb(out, v as i64);
}

fn f64_const(out: &mut Vec<u8>, v: f64) {
    put_u8(out, F64_CONST);
    out.extend_from_slice(&v.to_le_bytes());
}

fn emit_int_const(out: &mut Vec<u8>, v: i64, storage: u8) {
    if storage == VALTYPE_F64 {
        f64_const(out, v as f64);
    } else if v >= i32::MIN as i64 && v <= i32::MAX as i64 {
        i32_const(out, v as i32);
        put_u8(out, I64_EXTEND_I32_S);
    } else {
        i64_const(out, v);
    }
}

fn emit_float_const(out: &mut Vec<u8>, v: f64, storage: u8) {
    if storage == VALTYPE_F64 {
        f64_const(out, v);
    } else {
        emit_int_const(out, v as i64, VALTYPE_I64);
    }
}

fn convert(out: &mut Vec<u8>, from: u8, to: u8) {
    if from == to {
        return;
    }
    match (from, to) {
        (VALTYPE_I32, VALTYPE_I64) => put_u8(out, I64_EXTEND_I32_S),
        (VALTYPE_I64, VALTYPE_I32) => put_u8(out, I32_WRAP_I64),
        (VALTYPE_I64, VALTYPE_F64) => put_u8(out, F64_CONVERT_I64_S),
        (VALTYPE_F64, VALTYPE_I64) => put_u8(out, I64_TRUNC_F64_S),
        _ => {}
    }
}

fn emit_operand(
    out: &mut Vec<u8>,
    op: &Operand,
    regs: &Registers,
    storage: u8,
    loc: &SourceLoc,
    function: &str,
    diags: &mut Vec<Diagnostic>,
) {
    match op {
        Operand::Reg(name) => {
            if let Some(&index) = regs.map.get(name) {
                local_get(out, index);
                convert(out, regs.types[index as usize], storage);
            } else if let Some(&addr) = regs.consts.get(name) {
                i32_const(out, addr as i32);
                convert(out, crate::opcode::VALTYPE_I32, storage);
            } else {
                emit_int_const(out, 0, storage);
                diags.push(diag(
                    Trap::UnknownRegister,
                    &format!("register {name} is not defined"),
                    loc,
                    function,
                ));
            }
        }
        Operand::Int(v) => emit_int_const(out, *v as i64, storage),
        Operand::Float(v) => emit_float_const(out, *v, storage),
        Operand::Null | Operand::Undef | Operand::Label(_) => emit_int_const(out, 0, storage),
        Operand::Bytes(b) => emit_int_const(out, b.len() as i64, storage),
        Operand::Utf8(s) => emit_int_const(out, s.len() as i64, storage),
        Operand::Symbol(name) => {
            diags.push(diag(
                Trap::UnsupportedType,
                &format!("symbol operand {name} is not supported"),
                loc,
                function,
            ));
            emit_int_const(out, 0, storage);
        }
    }
}

fn operand_storage(op: Option<&Operand>, regs: &Registers, hint: Option<PrimType>) -> u8 {
    match op {
        Some(Operand::Reg(name)) => regs
            .map
            .get(name)
            .map(|&i| regs.types[i as usize])
            .unwrap_or(VALTYPE_I64),
        Some(Operand::Float(_)) => VALTYPE_F64,
        _ => storage_type(hint).unwrap_or(VALTYPE_I64),
    }
}

fn emit_address(
    out: &mut Vec<u8>,
    addr: &Address,
    regs: &Registers,
    loc: &SourceLoc,
    function: &str,
    diags: &mut Vec<Diagnostic>,
) {
    emit_operand(out, &addr.base, regs, VALTYPE_I64, loc, function, diags);
    put_u8(out, I32_WRAP_I64);
}

fn memarg(out: &mut Vec<u8>, align: u32, offset: i64) {
    put_uleb(out, align as u64);
    put_uleb(out, offset as u64);
}

#[derive(Clone, Copy)]
enum ConstructKind {
    Loop,
    Block,
}

#[derive(Clone)]
enum RegionItem {
    Block(usize),
    Loop(usize, usize),
}

impl RegionItem {
    fn first_block(&self) -> usize {
        match self {
            RegionItem::Block(i) | RegionItem::Loop(i, _) => *i,
        }
    }
    fn last_block(&self) -> usize {
        match self {
            RegionItem::Block(i) => *i,
            RegionItem::Loop(_, end) => *end,
        }
    }
}

struct ControlFlow {
    exit_construct: usize,
    loop_construct: HashMap<usize, usize>,
    forward_target: HashMap<usize, usize>,
    loop_range: HashMap<usize, usize>,
    stacks: Vec<Vec<usize>>,
    opens: Vec<Vec<ConstructKind>>,
    closes: Vec<usize>,
    next_construct: usize,
}

impl ControlFlow {
    fn new_construct(&mut self) -> usize {
        let id = self.next_construct;
        self.next_construct += 1;
        id
    }

    fn depth(&self, current: usize, target: &str, labels: &[String]) -> Option<u32> {
        let desired = if target.is_empty() || target == "exit" {
            self.exit_construct
        } else {
            let j = labels.iter().position(|l| l == target)?;
            if self
                .loop_range
                .get(&j)
                .map(|&end| j <= current && current <= end)
                .unwrap_or(false)
            {
                *self.loop_construct.get(&j)?
            } else {
                *self.forward_target.get(&j)?
            }
        };
        self.stacks[current]
            .iter()
            .position(|&id| id == desired)
            .map(|p| p as u32)
    }
}

fn build_items(lo: usize, hi: usize, ranges: &HashMap<usize, usize>) -> Vec<RegionItem> {
    let mut items = Vec::new();
    let mut i = lo;
    while i <= hi {
        if let Some(&end) = ranges.get(&i) {
            let end = end.min(hi);
            items.push(RegionItem::Loop(i, end));
            i = end + 1;
        } else {
            items.push(RegionItem::Block(i));
            i += 1;
        }
    }
    items
}

fn plan_region(
    items: &[RegionItem],
    region_header: usize,
    ranges: &HashMap<usize, usize>,
    enter_construct: usize,
    cf: &mut ControlFlow,
    stack: &mut Vec<usize>,
) {
    let loop_id = cf.new_construct();
    cf.loop_construct.insert(region_header, loop_id);
    cf.forward_target.insert(region_header, enter_construct);
    let first = items[0].first_block();
    cf.opens[first].push(ConstructKind::Loop);
    stack.insert(0, loop_id);

    let k = items.len();
    let mut block_ids: HashMap<usize, usize> = HashMap::new();
    for t in (1..k).rev() {
        let id = cf.new_construct();
        block_ids.insert(t, id);
        cf.forward_target.insert(items[t].first_block(), id);
        cf.opens[first].push(ConstructKind::Block);
        stack.insert(0, id);
    }

    for (t, item) in items.iter().enumerate() {
        match item {
            RegionItem::Block(i) => cf.stacks[*i] = stack.clone(),
            RegionItem::Loop(h, end) => {
                let mut children = vec![RegionItem::Block(*h)];
                if *h + 1 <= *end {
                    children.extend(build_items(*h + 1, *end, ranges));
                }
                let enter = if t > 0 { block_ids[&t] } else { loop_id };
                plan_region(&children, *h, ranges, enter, cf, stack);
            }
        }
        if let Some(&id) = block_ids.get(&(t + 1)) {
            let pos = stack.iter().position(|&x| x == id).expect("open construct");
            stack.remove(pos);
            cf.closes[items[t].last_block()] += 1;
        }
    }
    let pos = stack
        .iter()
        .position(|&x| x == loop_id)
        .expect("open loop construct");
    stack.remove(pos);
    cf.closes[items[k - 1].last_block()] += 1;
}

fn build_control_flow(func: &Function, labels: &[String]) -> ControlFlow {
    let n = func.blocks.len();
    let mut ranges: HashMap<usize, usize> = HashMap::new();
    for (i, block) in func.blocks.iter().enumerate() {
        for ins in &block.instrs {
            let targets: Vec<&str> = match &ins.kind {
                InstructionKind::Jmp { target } => vec![target],
                InstructionKind::Br {
                    then_label,
                    else_label,
                    ..
                } => vec![then_label, else_label],
                InstructionKind::BrNull { target, .. } => vec![target],
                _ => Vec::new(),
            };
            for target in targets {
                if let Some(j) = labels.iter().position(|l| l == target) {
                    if j < i {
                        ranges
                            .entry(j)
                            .and_modify(|e| *e = (*e).max(i))
                            .or_insert(i);
                    }
                }
            }
        }
    }
    let mut headers: Vec<usize> = ranges.keys().copied().collect();
    headers.sort_unstable();
    let mut prev = 0usize;
    for h in headers {
        let end = ranges.get_mut(&h).expect("header range");
        *end = (*end).max(prev);
        prev = *end;
    }

    let mut cf = ControlFlow {
        exit_construct: 0,
        loop_construct: HashMap::new(),
        forward_target: HashMap::new(),
        loop_range: ranges.clone(),
        stacks: vec![Vec::new(); n],
        opens: vec![Vec::new(); n],
        closes: vec![0; n],
        next_construct: 1,
    };
    let mut stack = vec![cf.exit_construct];
    if n > 0 {
        let items = build_items(0, n - 1, &ranges);
        plan_region(&items, 0, &ranges, cf.exit_construct, &mut cf, &mut stack);
    }
    cf
}

fn emit_branch(
    out: &mut Vec<u8>,
    target: &str,
    labels: &[String],
    cf: &ControlFlow,
    current: usize,
    conditional: bool,
) {
    match cf.depth(current, target, labels) {
        Some(depth) => {
            put_u8(out, if conditional { BR_IF } else { BR });
            put_uleb(out, depth as u64);
        }
        None => {
            if conditional {
                put_u8(out, DROP);
            }
            put_u8(out, UNREACHABLE);
        }
    }
}

fn condition_to_i32(out: &mut Vec<u8>) {
    put_u8(out, I64_EQZ);
    put_u8(out, I32_EQZ);
}

fn numeric_opcode(op: OpKind, storage: u8, ty: Option<PrimType>) -> Option<u8> {
    let float = storage == VALTYPE_F64;
    let signed = is_signed_prim(ty);
    let code = match op {
        OpKind::Add | OpKind::FAdd => {
            if float {
                F64_ADD
            } else {
                I64_ADD
            }
        }
        OpKind::Sub | OpKind::FSub => {
            if float {
                F64_SUB
            } else {
                I64_SUB
            }
        }
        OpKind::Mul | OpKind::FMul => {
            if float {
                F64_MUL
            } else {
                I64_MUL
            }
        }
        OpKind::Div | OpKind::SDiv | OpKind::FDiv => {
            if float {
                F64_DIV
            } else if signed || matches!(op, OpKind::SDiv) {
                I64_DIV_S
            } else {
                I64_DIV_U
            }
        }
        OpKind::UDiv => I64_DIV_U,
        OpKind::Rem | OpKind::SRem => {
            if signed || matches!(op, OpKind::SRem) {
                I64_REM_S
            } else {
                I64_REM_U
            }
        }
        OpKind::URem => I64_REM_U,
        OpKind::And => I64_AND,
        OpKind::Or => I64_OR,
        OpKind::Xor => I64_XOR,
        OpKind::Shl => I64_SHL,
        OpKind::LShr | OpKind::Shr => I64_SHR_U,
        OpKind::AShr => I64_SHR_S,
        OpKind::Eq | OpKind::FcmpEq => {
            if float {
                F64_EQ
            } else {
                I64_EQ
            }
        }
        OpKind::Ne | OpKind::FcmpNe => {
            if float {
                F64_NE
            } else {
                I64_NE
            }
        }
        OpKind::Slt | OpKind::Lt | OpKind::FcmpLt => {
            if float {
                F64_LT
            } else if signed || matches!(op, OpKind::Slt) {
                I64_LT_S
            } else {
                I64_LT_U
            }
        }
        OpKind::Sle | OpKind::FcmpLe => {
            if float {
                F64_LE
            } else {
                I64_LE_S
            }
        }
        OpKind::Sgt | OpKind::Gt | OpKind::FcmpGt => {
            if float {
                F64_GT
            } else if signed || matches!(op, OpKind::Sgt) {
                I64_GT_S
            } else {
                I64_GT_U
            }
        }
        OpKind::Sge | OpKind::FcmpGe => {
            if float {
                F64_GE
            } else {
                I64_GE_S
            }
        }
        OpKind::Ult => I64_LT_U,
        OpKind::Ule => I64_LE_U,
        OpKind::Ugt => I64_GT_U,
        OpKind::Uge => I64_GE_U,
        OpKind::Fptosi => I64_TRUNC_F64_S,
        OpKind::Sitofp => F64_CONVERT_I64_S,
        OpKind::Uitofp => F64_CONVERT_I64_U,
        OpKind::Fpext | OpKind::Fptrunc => return None,
        OpKind::Trunc | OpKind::Zext | OpKind::Sext | OpKind::Bitcast => return None,
        OpKind::Neg | OpKind::FNeg | OpKind::Not => return None,
        OpKind::AddV128
        | OpKind::SubV128
        | OpKind::MulV128
        | OpKind::ShuffleV128
        | OpKind::ExtractLane
        | OpKind::InsertLane => return None,
    };
    Some(code)
}

fn lower_op(
    out: &mut Vec<u8>,
    ins: &Instruction,
    op: OpKind,
    args: &[Operand],
    regs: &Registers,
    dest_storage: u8,
    function: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let loc = &ins.loc;
    let storage = match op {
        OpKind::Sitofp | OpKind::Uitofp => VALTYPE_I64,
        OpKind::Fptosi => VALTYPE_F64,
        _ => operand_storage(args.first(), regs, ins.ty),
    };
    let comparison = op.is_comparison();
    let conversion = op.is_conversion();
    let mut result_storage = storage;
    match op {
        OpKind::AddV128
        | OpKind::SubV128
        | OpKind::MulV128
        | OpKind::ShuffleV128
        | OpKind::ExtractLane
        | OpKind::InsertLane => {
            diags.push(diag(
                Trap::UnsupportedType,
                "v128 operations are not supported",
                loc,
                function,
            ));
            return;
        }
        OpKind::Neg | OpKind::FNeg => {
            if storage == VALTYPE_F64 {
                if let Some(a) = args.first() {
                    emit_operand(out, a, regs, storage, loc, function, diags);
                }
                put_u8(out, F64_NEG);
            } else {
                emit_int_const(out, 0, VALTYPE_I64);
                if let Some(a) = args.first() {
                    emit_operand(out, a, regs, VALTYPE_I64, loc, function, diags);
                }
                put_u8(out, I64_SUB);
                result_storage = VALTYPE_I64;
            }
        }
        OpKind::Not => {
            emit_int_const(out, -1, VALTYPE_I64);
            if let Some(a) = args.first() {
                emit_operand(out, a, regs, VALTYPE_I64, loc, function, diags);
            }
            put_u8(out, I64_XOR);
            result_storage = VALTYPE_I64;
        }
        _ => {
            if args.len() == 3 {
                diags.push(diag(
                    Trap::UnsupportedInstruction,
                    "ternary operations are not supported",
                    loc,
                    function,
                ));
                return;
            }
            for a in args {
                emit_operand(out, a, regs, storage, loc, function, diags);
            }
            if let Some(code) = numeric_opcode(op, storage, ins.ty) {
                put_u8(out, code);
                result_storage = match op {
                    OpKind::Fptosi => VALTYPE_I64,
                    OpKind::Sitofp | OpKind::Uitofp => VALTYPE_F64,
                    _ => storage,
                };
            } else if !conversion {
                diags.push(diag(
                    Trap::UnsupportedInstruction,
                    "operation cannot be lowered",
                    loc,
                    function,
                ));
                return;
            }
            if comparison {
                put_u8(out, I64_EXTEND_I32_U);
                result_storage = VALTYPE_I64;
            }
        }
    }
    convert(out, result_storage, dest_storage);
}

fn call_sig<'a>(program: &'a Program, name: &str) -> Option<&'a Signature> {
    program.externs.iter().find(|e| e.name == name).or_else(|| {
        program
            .functions
            .iter()
            .find(|f| f.sig.name == name)
            .map(|f| &f.sig)
    })
}

fn indirect_type_index(types: &[Vec<u8>], params: &[u8], results: &[u8]) -> Option<u32> {
    let encoded = functype(params, results);
    types.iter().position(|t| *t == encoded).map(|i| i as u32)
}

fn lower_instruction(
    out: &mut Vec<u8>,
    ins: &Instruction,
    regs: &Registers,
    func_index: &HashMap<String, u32>,
    types: &[Vec<u8>],
    labels: &[String],
    cf: &ControlFlow,
    current: usize,
    program: &Program,
    function: &str,
    ret_storage: u8,
    return_fallible: bool,
    diags: &mut Vec<Diagnostic>,
) {
    let loc = &ins.loc;
    if ins.ty == Some(PrimType::V128) {
        diags.push(diag(
            Trap::UnsupportedType,
            "v128 is not supported",
            loc,
            function,
        ));
        return;
    }
    let dest_storage = ins
        .dest
        .as_ref()
        .and_then(|d| regs.map.get(d))
        .map(|&i| regs.types[i as usize])
        .unwrap_or(VALTYPE_I64);
    let set_dest = |out: &mut Vec<u8>| {
        if let Some(name) = &ins.dest {
            if let Some(&index) = regs.map.get(name) {
                local_set(out, index);
            }
        }
    };
    match &ins.kind {
        InstructionKind::Alloc { size } | InstructionKind::StackAlloc { size } => {
            match size {
                Operand::Bytes(b) => emit_int_const(out, b.len() as i64, VALTYPE_I64),
                Operand::Utf8(s) => emit_int_const(out, s.len() as i64, VALTYPE_I64),
                _ => emit_operand(out, size, regs, VALTYPE_I64, loc, function, diags),
            }
            local_set(out, regs.scratch[1]);
            i32_const(out, 0);
            put_u8(out, I64_LOAD);
            memarg(out, 3, 0);
            local_set(out, regs.scratch[0]);
            local_get(out, regs.scratch[0]);
            local_get(out, regs.scratch[1]);
            put_u8(out, I64_ADD);
            local_set(out, regs.scratch[1]);
            local_get(out, regs.scratch[0]);
            if ins.dest.is_some() {
                set_dest(out);
            } else {
                put_u8(out, DROP);
            }
            i32_const(out, 0);
            local_get(out, regs.scratch[1]);
            put_u8(out, I64_STORE);
            memarg(out, 3, 0);
        }
        InstructionKind::Load { addr } => {
            let (code, align) = match load_opcode(ins.ty) {
                Some(v) => v,
                None => {
                    diags.push(diag(
                        Trap::UnsupportedType,
                        "load type is not supported",
                        loc,
                        function,
                    ));
                    return;
                }
            };
            emit_address(out, addr, regs, loc, function, diags);
            put_u8(out, code);
            memarg(out, align, addr.offset);
            match valtype_for_prim(ins.ty.unwrap_or(PrimType::I64)) {
                Some(VALTYPE_I32) => {
                    put_u8(
                        out,
                        if is_signed_prim(ins.ty) {
                            I64_EXTEND_I32_S
                        } else {
                            I64_EXTEND_I32_U
                        },
                    );
                }
                Some(VALTYPE_F32) => put_u8(out, F64_PROMOTE_F32),
                _ => {}
            }
            store_or_drop(out, ins, regs);
        }
        InstructionKind::Store { addr, value } => {
            let (code, align) = match store_opcode(ins.ty) {
                Some(v) => v,
                None => {
                    diags.push(diag(
                        Trap::UnsupportedType,
                        "store type is not supported",
                        loc,
                        function,
                    ));
                    return;
                }
            };
            emit_address(out, addr, regs, loc, function, diags);
            let mem_ty = valtype_for_prim(ins.ty.unwrap_or(PrimType::I64)).unwrap_or(VALTYPE_I64);
            let value_storage = operand_storage(Some(value), regs, ins.ty);
            emit_operand(out, value, regs, value_storage, loc, function, diags);
            match (value_storage, mem_ty) {
                (VALTYPE_I64, VALTYPE_F64) => put_u8(out, F64_CONVERT_I64_S),
                (VALTYPE_I64, VALTYPE_F32) => {
                    put_u8(out, F64_CONVERT_I64_S);
                    put_u8(out, F32_DEMOTE_F64);
                }
                (VALTYPE_F64, VALTYPE_I32) => {
                    put_u8(out, I64_TRUNC_F64_S);
                    put_u8(out, I32_WRAP_I64);
                }
                (VALTYPE_F64, VALTYPE_I64) => put_u8(out, I64_TRUNC_F64_S),
                (VALTYPE_F64, VALTYPE_F32) => put_u8(out, F32_DEMOTE_F64),
                (VALTYPE_I64, VALTYPE_I32) => put_u8(out, I32_WRAP_I64),
                _ => {}
            }
            put_u8(out, code);
            memarg(out, align, addr.offset);
        }
        InstructionKind::Op { op, args } => {
            lower_op(out, ins, *op, args, regs, dest_storage, function, diags);
            store_or_drop(out, ins, regs);
        }
        InstructionKind::Jmp { target } => {
            emit_branch(out, target, labels, cf, current, false);
        }
        InstructionKind::Br {
            cond,
            then_label,
            else_label,
        } => {
            emit_operand(out, cond, regs, VALTYPE_I64, loc, function, diags);
            condition_to_i32(out);
            emit_branch(out, then_label, labels, cf, current, true);
            emit_branch(out, else_label, labels, cf, current, false);
        }
        InstructionKind::BrNull { cond, target, .. } => {
            emit_operand(out, cond, regs, VALTYPE_I64, loc, function, diags);
            put_u8(out, I64_EQZ);
            emit_branch(out, target, labels, cf, current, true);
        }
        InstructionKind::Call { target, args } => {
            let sig = match call_sig(program, target) {
                Some(s) => s,
                None => {
                    diags.push(diag(
                        Trap::ImportResolutionFailed,
                        &format!("call to unknown function {target}"),
                        loc,
                        function,
                    ));
                    return;
                }
            };
            for (arg, param) in args.iter().zip(sig.params.iter()) {
                let storage = storage_type(Some(param.ty)).unwrap_or(VALTYPE_I64);
                emit_operand(out, &arg.value, regs, storage, loc, function, diags);
            }
            if let Some(&index) = func_index.get(target) {
                put_u8(out, CALL);
                put_uleb(out, index as u64);
                if sig.return_fallible {
                    if !sig.return_ty.is_void() {
                        store_or_drop(out, ins, regs);
                    }
                    set_status(out, ins.dest.as_deref(), regs);
                } else if !sig.return_ty.is_void() {
                    store_or_drop(out, ins, regs);
                }
            }
        }
        InstructionKind::CallIndirect { target, args } => {
            let mut params = Vec::with_capacity(args.len());
            for arg in args {
                let storage = operand_storage(Some(&arg.value), regs, None);
                params.push(storage);
                emit_operand(out, &arg.value, regs, storage, loc, function, diags);
            }
            let results: Vec<u8> = if let Some(&dropped) = regs
                .indirect_drops
                .get(&(ins as *const Instruction as usize))
            {
                vec![dropped]
            } else if ins.dest.is_some() {
                vec![dest_storage]
            } else {
                Vec::new()
            };
            let type_index = match indirect_type_index(types, &params, &results) {
                Some(index) => index,
                None => {
                    diags.push(diag(
                        Trap::UnsupportedType,
                        "indirect call signature is not available",
                        loc,
                        function,
                    ));
                    return;
                }
            };
            if let Operand::Int(value) = target {
                if *value < SA_FN_BASE as i128 {
                    i32_const(out, *value as i32);
                } else {
                    emit_operand(out, target, regs, VALTYPE_I64, loc, function, diags);
                    i64_const(out, SA_FN_BASE as i64);
                    put_u8(out, I64_SUB);
                    put_u8(out, I32_WRAP_I64);
                }
            } else {
                emit_operand(out, target, regs, VALTYPE_I64, loc, function, diags);
                i64_const(out, SA_FN_BASE as i64);
                put_u8(out, I64_SUB);
                put_u8(out, I32_WRAP_I64);
            }
            put_u8(out, CALL_INDIRECT);
            put_uleb(out, type_index as u64);
            put_uleb(out, 0);
            if ins.dest.is_some()
                || regs
                    .indirect_drops
                    .contains_key(&(ins as *const Instruction as usize))
            {
                store_or_drop(out, ins, regs);
            }
        }
        InstructionKind::Return { value } | InstructionKind::EarlyReturn { value } => {
            if return_fallible {
                if let Some(v) = value {
                    emit_status(out, v, regs);
                    emit_operand(out, v, regs, ret_storage, loc, function, diags);
                } else {
                    i32_const(out, 0);
                }
                put_u8(out, RETURN);
                put_u8(out, UNREACHABLE);
            } else if let Some(v) = value {
                emit_operand(out, v, regs, ret_storage, loc, function, diags);
                put_u8(out, RETURN);
                put_u8(out, UNREACHABLE);
            } else {
                emit_branch(out, "exit", labels, cf, current, false);
            }
        }
        InstructionKind::Try { src } => {
            let status_index = match src {
                Operand::Reg(name) => regs.status.get(name).copied(),
                _ => None,
            };
            if let Some(index) = status_index {
                local_get(out, index);
                put_u8(out, IF);
                out.push(0x40);
                if return_fallible {
                    local_get(out, index);
                    emit_operand(out, src, regs, ret_storage, loc, function, diags);
                    put_u8(out, RETURN);
                    put_u8(out, UNREACHABLE);
                } else {
                    put_u8(out, UNREACHABLE);
                }
                put_u8(out, END);
            }
            if ins.dest.is_some() {
                emit_operand(out, src, regs, dest_storage, loc, function, diags);
                set_dest(out);
                i32_const(out, 0);
                set_status(out, ins.dest.as_deref(), regs);
            }
        }
        InstructionKind::Assign { src }
        | InstructionKind::Move { src }
        | InstructionKind::RawCast { src }
        | InstructionKind::AssumeSafe { src }
        | InstructionKind::Borrow { src, .. }
        | InstructionKind::AssumeBorrow { src, .. }
        | InstructionKind::VaArg { src } => {
            if ins.dest.is_some() {
                emit_operand(out, src, regs, dest_storage, loc, function, diags);
                set_dest(out);
                if matches!(
                    ins.kind,
                    InstructionKind::Assign { .. } | InstructionKind::Move { .. }
                ) {
                    emit_status(out, src, regs);
                    set_status(out, ins.dest.as_deref(), regs);
                }
            }
        }
        InstructionKind::PtrAdd { base, offset } => {
            emit_operand(out, base, regs, VALTYPE_I64, loc, function, diags);
            emit_operand(out, offset, regs, VALTYPE_I64, loc, function, diags);
            put_u8(out, I64_ADD);
            store_or_drop(out, ins, regs);
        }
        InstructionKind::AtomicLoad { .. }
        | InstructionKind::AtomicStore { .. }
        | InstructionKind::Cmpxchg { .. }
        | InstructionKind::AtomicRmw { .. }
        | InstructionKind::Fence { .. } => {
            diags.push(diag(
                Trap::UnsupportedInstruction,
                "atomic operations require target atomics support",
                loc,
                function,
            ));
        }
        InstructionKind::Panic { .. } => put_u8(out, UNREACHABLE),
        InstructionKind::Take { .. }
        | InstructionKind::Release { .. }
        | InstructionKind::VaStart
        | InstructionKind::VaEnd
        | InstructionKind::Native { .. }
        | InstructionKind::Nop => {}
    }
}

pub fn lower_function(
    program: &Program,
    func: &Function,
    func_index: &HashMap<String, u32>,
    types: &[Vec<u8>],
    entry: &str,
    heap_start: i64,
    diags: &mut Vec<Diagnostic>,
) -> FunctionCode {
    let regs = collect_registers(func, program, diags);
    let mut locals = Vec::new();
    for (index, ty) in regs.types.iter().enumerate() {
        if index < func.sig.params.len() {
            continue;
        }
        locals.push((1, *ty));
    }
    let mut code = Vec::new();
    if func.sig.name == entry {
        i32_const(&mut code, 0);
        emit_int_const(&mut code, heap_start, VALTYPE_I64);
        put_u8(&mut code, I64_STORE);
        memarg(&mut code, 3, 0);
    }
    for &index in regs.status.values() {
        i32_const(&mut code, 0);
        local_set(&mut code, index);
    }
    let blocks = &func.blocks;
    let n = blocks.len();
    let labels: Vec<String> = blocks.iter().map(|b| b.label.clone()).collect();
    let ret_storage = storage_type(Some(func.sig.return_value_type())).unwrap_or(VALTYPE_I64);
    if n > 0 {
        let cf = build_control_flow(func, &labels);
        put_u8(&mut code, BLOCK);
        code.push(0x40);
        for (i, block) in blocks.iter().enumerate() {
            for kind in &cf.opens[i] {
                put_u8(
                    &mut code,
                    match kind {
                        ConstructKind::Loop => LOOP,
                        ConstructKind::Block => BLOCK,
                    },
                );
                code.push(0x40);
            }
            for ins in &block.instrs {
                lower_instruction(
                    &mut code,
                    ins,
                    &regs,
                    func_index,
                    types,
                    &labels,
                    &cf,
                    i,
                    program,
                    func.name(),
                    ret_storage,
                    func.sig.return_fallible,
                    diags,
                );
            }
            for _ in 0..cf.closes[i] {
                put_u8(&mut code, END);
            }
        }
        put_u8(&mut code, END);
    }
    if func.sig.return_fallible {
        i32_const(&mut code, 0);
    }
    if !func.sig.return_ty.is_void() {
        emit_int_const(&mut code, 0, ret_storage);
    }
    put_u8(&mut code, END);
    FunctionCode { locals, code }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn func_with_jumps(labels: &[&str], jumps: &[(usize, &str)]) -> (Function, Vec<String>) {
        let mut f = Function::new(Signature::new("test"), SourceLoc::unknown());
        for label in labels {
            f.blocks
                .push(sa_ir::Block::new(*label, SourceLoc::unknown()));
        }
        for &(i, target) in jumps {
            f.blocks[i].instrs.push(Instruction::new(
                InstructionKind::Jmp {
                    target: target.to_string(),
                },
                SourceLoc::unknown(),
                "jmp",
            ));
        }
        let labels: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
        (f, labels)
    }

    #[test]
    fn forward_branches_get_block_end_depths() {
        let (f, labels) = func_with_jumps(&["a", "b", "c"], &[(0, "c"), (1, "c")]);
        let cf = build_control_flow(&f, &labels);
        assert_eq!(cf.depth(0, "c", &labels), Some(1));
        assert_eq!(cf.depth(1, "c", &labels), Some(0));
        assert_eq!(cf.depth(0, "exit", &labels), Some(3));
        assert_eq!(cf.depth(2, "exit", &labels), Some(1));
    }

    #[test]
    fn backedge_to_nonzero_header_branches_to_its_loop() {
        let (f, labels) = func_with_jumps(
            &["entry", "cond", "body"],
            &[(0, "cond"), (1, "body"), (2, "cond")],
        );
        let cf = build_control_flow(&f, &labels);
        assert_eq!(cf.depth(2, "cond", &labels), Some(0));
        assert_eq!(cf.depth(1, "body", &labels), Some(0));
        assert_eq!(cf.depth(0, "cond", &labels), Some(0));
        assert_eq!(cf.depth(1, "exit", &labels), Some(3));
    }

    #[test]
    fn backedge_lowering_emits_loop_instead_of_unreachable() {
        let (f, _) = func_with_jumps(&["entry", "cond", "body"], &[(2, "cond")]);
        let mut diags = Vec::new();
        let code =
            lower_function(&Program::new(), &f, &HashMap::new(), &[], "", 0, &mut diags).code;
        assert!(diags.is_empty());
        assert!(code.contains(&LOOP));
        assert!(code.windows(2).any(|w| w[0] == BR && w[1] == 0));
        assert!(!code.windows(2).any(|w| w[0] == DROP && w[1] == UNREACHABLE));
        let opens = code
            .windows(2)
            .filter(|w| (w[0] == LOOP || w[0] == BLOCK) && w[1] == 0x40)
            .count();
        let ends = code.iter().filter(|&&b| b == END).count();
        assert_eq!(opens + 1, ends);
    }

    #[test]
    fn fallible_call_and_try_emit_status_checks() {
        let mut program = Program::new();
        let mut ext = Signature::new("sa_maybe");
        ext.return_fallible = true;
        ext.return_ty = PrimType::I64;
        program.externs.push(ext);
        let mut sig = Signature::new("main");
        sig.return_fallible = true;
        sig.return_ty = PrimType::I32;
        let mut f = Function::new(sig, SourceLoc::unknown());
        let mut block = sa_ir::Block::new("entry", SourceLoc::unknown());
        block.instrs.push(
            Instruction::new(
                InstructionKind::Call {
                    target: "sa_maybe".to_string(),
                    args: Vec::new(),
                },
                SourceLoc::unknown(),
                "call",
            )
            .with_dest("r")
            .with_ty(PrimType::I64),
        );
        block.instrs.push(
            Instruction::new(
                InstructionKind::Try {
                    src: Operand::reg("r"),
                },
                SourceLoc::unknown(),
                "?",
            )
            .with_dest("v")
            .with_ty(PrimType::I64),
        );
        f.blocks.push(block);
        program.functions.push(f);

        let mut func_index = HashMap::new();
        func_index.insert("sa_maybe".to_string(), 0u32);
        let types = vec![functype(&[], &[VALTYPE_I32, VALTYPE_I64])];
        let mut diags = Vec::new();
        let code = lower_function(
            &program,
            &program.functions[0],
            &func_index,
            &types,
            "",
            0,
            &mut diags,
        )
        .code;
        assert!(diags.is_empty());
        assert!(code.windows(2).any(|w| w[0] == CALL && w[1] == 0));
        assert!(code.contains(&IF));
        assert!(code.contains(&RETURN));
        assert!(code.iter().filter(|&&b| b == LOCAL_SET).count() >= 4);
    }

    #[test]
    fn extra_destinations_get_wasm_locals() {
        let mut sig = Signature::new("main");
        sig.return_ty = PrimType::I32;
        let mut f = Function::new(sig, SourceLoc::unknown());
        let mut entry = sa_ir::Block::new("entry", SourceLoc::unknown());
        let mut exchange = Instruction::new(
            InstructionKind::Cmpxchg {
                addr: Address::new(Operand::reg("p"), 0),
                expected: Operand::Int(0),
                replacement: Operand::Int(1),
                order: sa_ir::AtomicOrdering::SeqCst,
            },
            SourceLoc::unknown(),
            "old, ok = cmpxchg",
        );
        exchange.dest = Some("old".into());
        exchange.extra_dests = vec!["ok".into()];
        entry.instrs.push(exchange);
        f.blocks.push(entry);
        let mut diags = Vec::new();
        let regs = collect_registers(&f, &Program::new(), &mut diags);
        assert!(diags.is_empty());
        assert!(regs.map.contains_key("old"));
        assert!(regs.map.contains_key("ok"));
    }

    #[test]
    fn numeric_opcodes_map() {
        assert_eq!(
            numeric_opcode(OpKind::Add, VALTYPE_I64, None),
            Some(I64_ADD)
        );
        assert_eq!(
            numeric_opcode(OpKind::Add, VALTYPE_F64, None),
            Some(F64_ADD)
        );
        assert_eq!(
            numeric_opcode(OpKind::UDiv, VALTYPE_I64, None),
            Some(I64_DIV_U)
        );
        assert_eq!(numeric_opcode(OpKind::Eq, VALTYPE_I64, None), Some(I64_EQ));
    }
}
