use crate::memory::Memory;
use crate::value::apply_op;
use crate::{InterpOptions, RuntimeError, Value};
use sa_ir::{
    Address, AtomicRmwOp, CapPrefix, ConstValue, Function, Instruction, InstructionKind, Operand,
    OpKind, PrimType, Program, Trap,
};
use std::collections::HashMap;

const MAX_DEPTH: usize = 256;

struct Frame<'a> {
    func: &'a Function,
    regs: HashMap<String, Value>,
    block: usize,
    instr: usize,
    va: Vec<Value>,
    va_pos: usize,
}

impl<'a> Frame<'a> {
    fn new(func: &'a Function) -> Self {
        Self {
            func,
            regs: HashMap::new(),
            block: 0,
            instr: 0,
            va: Vec::new(),
            va_pos: 0,
        }
    }
}

pub struct Interpreter<'a> {
    program: &'a Program,
    opts: InterpOptions,
    memory: Memory,
    output: Vec<u8>,
    trace: Vec<String>,
    steps: u64,
    gas: u64,
    depth: usize,
}

impl<'a> Interpreter<'a> {
    pub fn new(program: &'a Program, opts: InterpOptions) -> Self {
        Self {
            program,
            opts,
            memory: Memory::new(1024 * 1024),
            output: Vec::new(),
            trace: Vec::new(),
            steps: 0,
            gas: 0,
            depth: 0,
        }
    }

    pub fn run(&mut self) -> Result<i32, RuntimeError> {
        let entry = self
            .program
            .entry_function()
            .cloned()
            .ok_or_else(|| RuntimeError::new(Trap::RuntimeError, "no entry function"))?;
        let value = self.call_function(&entry, Vec::new())?;
        Ok(match value {
            Value::Int(v) => v as i32,
            Value::Float(v) => v as i32,
            Value::Ptr(v) => v as i32,
            Value::Bytes(_) | Value::Void => 0,
        })
    }

    pub fn run_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        self.call(name, args)
    }

    pub fn last_output(&self) -> &[u8] {
        &self.output
    }

    pub fn trace_log(&self) -> &[String] {
        &self.trace
    }

    pub fn gas_used(&self) -> u64 {
        self.gas
    }

    pub fn steps_used(&self) -> u64 {
        self.steps
    }

    fn tick(&mut self, ins: &Instruction) -> Result<(), RuntimeError> {
        self.steps += 1;
        self.gas = self.gas.saturating_add(ins.gas_cost());
        if self.steps > self.opts.max_steps || self.gas > self.opts.gas_limit {
            return Err(RuntimeError::new(Trap::GasExceeded, "step or gas limit exceeded"));
        }
        if self.opts.trace {
            self.trace.push(format!("{}: {}", ins.loc, ins.to_text()));
        }
        Ok(())
    }

    fn call(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let n = normalize(name);
        let program = self.program;
        if let Some(func) = program
            .function(n)
            .or_else(|| program.functions.iter().find(|f| normalize(&f.sig.name) == n))
        {
            return self.call_function(func, args);
        }
        let is_extern = program.has_extern(name)
            || program.has_extern(n)
            || program.externs.iter().any(|e| normalize(&e.name) == n)
            || name.starts_with('@')
            || n.starts_with("sa_");
        if is_extern {
            return self.call_extern(n, &args);
        }
        Err(RuntimeError::new(
            Trap::RuntimeError,
            format!("unknown function: {name}"),
        ))
    }

    fn call_extern(&mut self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        match name {
            "sa_print_bytes" => {
                let ptr = args.first().map(|v| v.as_u64().unwrap_or(0)).unwrap_or(0);
                let len = args
                    .get(1)
                    .map(|v| v.as_u64().unwrap_or(0) as usize)
                    .unwrap_or(0);
                let bytes = self.memory.read(ptr, len)?.to_vec();
                self.output.extend_from_slice(&bytes);
                Ok(Value::Void)
            }
            "sa_print_i64" | "sa_print_i32" => {
                let n = args.first().map(Value::to_i128).unwrap_or(0);
                self.output.extend_from_slice(n.to_string().as_bytes());
                Ok(Value::Void)
            }
            _ => Ok(Value::Int(0)),
        }
    }

    fn call_function(&mut self, func: &Function, args: Vec<Value>) -> Result<Value, RuntimeError> {
        if self.depth >= MAX_DEPTH {
            return Err(RuntimeError::new(
                Trap::RuntimeError,
                "call depth limit exceeded",
            ));
        }
        self.depth += 1;
        let mut frame = Frame::new(func);
        let mut rest = args.into_iter();
        for param in &func.sig.params {
            let value = rest.next().unwrap_or(Value::Void);
            frame.regs.insert(param.name.clone(), value);
        }
        if func.sig.is_variadic {
            frame.va = rest.collect();
        }
        let result = self.exec_frame(&mut frame);
        self.depth -= 1;
        result
    }

    fn exec_frame(&mut self, frame: &mut Frame<'_>) -> Result<Value, RuntimeError> {
        loop {
            let func = frame.func;
            let block = match func.blocks.get(frame.block) {
                Some(b) => b,
                None => return Ok(Value::Void),
            };
            if frame.instr >= block.instrs.len() {
                frame.block += 1;
                frame.instr = 0;
                continue;
            }
            let ins = &block.instrs[frame.instr];
            frame.instr += 1;
            self.tick(ins)?;
            match &ins.kind {
                InstructionKind::Alloc { size } | InstructionKind::StackAlloc { size } => {
                    let n = self.resolve(frame, size)?.as_u64().unwrap_or(0) as usize;
                    let ptr = self.memory.alloc(n, 8)?;
                    set_dest(frame, ins, Value::Ptr(ptr));
                }
                InstructionKind::Load { addr } => {
                    let ptr = self.resolve_addr(frame, addr)?;
                    let ty = ins.ty.unwrap_or(PrimType::I64);
                    let value = self.memory.load(ptr, ty)?;
                    set_dest(frame, ins, value);
                }
                InstructionKind::Store { addr, value } => {
                    let ptr = self.resolve_addr(frame, addr)?;
                    let value = self.resolve(frame, value)?;
                    let ty = ins.ty.unwrap_or(infer_ty(&value));
                    self.memory.store(ptr, ty, &value)?;
                }
                InstructionKind::Op { op, args } => {
                    let mut vals = Vec::with_capacity(args.len());
                    for arg in args {
                        vals.push(self.resolve(frame, arg)?);
                    }
                    let value = apply_op(*op, &vals, ins.ty)?;
                    set_dest(frame, ins, value);
                }
                InstructionKind::Jmp { target } => self.jump(frame, target)?,
                InstructionKind::Br {
                    cond,
                    then_label,
                    else_label,
                } => {
                    let value = self.resolve(frame, cond)?;
                    let label = if value.is_truthy() {
                        then_label
                    } else {
                        else_label
                    };
                    self.jump(frame, label)?;
                }
                InstructionKind::BrNull { cond, target, else_target } => {
                    let value = self.resolve(frame, cond)?;
                    if matches!(value, Value::Ptr(0) | Value::Int(0) | Value::Void) {
                        self.jump(frame, target)?;
                    } else if let Some(other) = else_target {
                        self.jump(frame, other)?;
                    }
                }
                InstructionKind::Call { target, args } => {
                    let mut vals = Vec::with_capacity(args.len());
                    for arg in args {
                        vals.push(self.resolve_arg(frame, arg)?);
                    }
                    let value = self.call(target, vals)?;
                    set_dest(frame, ins, value);
                }
                InstructionKind::CallIndirect { target, args } => {
                    let mut vals = Vec::with_capacity(args.len());
                    for arg in args {
                        vals.push(self.resolve_arg(frame, arg)?);
                    }
                    let value = match target {
                        Operand::Symbol(name) => self.call(name, vals)?,
                        _ => {
                            let target_value = self.resolve(frame, target)?;
                            let index = target_value.as_u64().unwrap_or(0) as usize;
                            match self.program.functions.get(index) {
                                Some(func) => self.call_function(func, vals)?,
                                None => {
                                    return Err(RuntimeError::new(
                                        Trap::RuntimeError,
                                        format!("invalid indirect call target {index}"),
                                    ));
                                }
                            }
                        }
                    };
                    set_dest(frame, ins, value);
                }
                InstructionKind::Return { value } | InstructionKind::EarlyReturn { value } => {
                    return match value {
                        Some(value) => self.resolve(frame, value),
                        None => Ok(Value::Void),
                    };
                }
                InstructionKind::Take { addr } => {
                    let ptr = self.resolve_addr(frame, addr)?;
                    let ty = ins.ty.unwrap_or(PrimType::I64);
                    let value = self.memory.load(ptr, ty)?;
                    set_dest(frame, ins, value);
                }
                InstructionKind::Release { .. } => {}
                InstructionKind::Assign { src }
                | InstructionKind::Move { src }
                | InstructionKind::Borrow { src, .. }
                | InstructionKind::RawCast { src }
                | InstructionKind::AssumeSafe { src }
                | InstructionKind::AssumeBorrow { src, .. } => {
                    let value = self.resolve(frame, src)?;
                    set_dest(frame, ins, value);
                }
                InstructionKind::PtrAdd { base, offset } => {
                    let b = self.resolve(frame, base)?.as_u64().unwrap_or(0);
                    let o = self.resolve(frame, offset)?.as_u64().unwrap_or(0);
                    set_dest(frame, ins, Value::Ptr(b.wrapping_add(o)));
                }
                InstructionKind::AtomicLoad { addr, .. } => {
                    let ptr = self.resolve_addr(frame, addr)?;
                    let ty = ins.ty.unwrap_or(PrimType::I64);
                    let value = self.memory.load(ptr, ty)?;
                    set_dest(frame, ins, value);
                }
                InstructionKind::AtomicStore { addr, value, .. } => {
                    let ptr = self.resolve_addr(frame, addr)?;
                    let value = self.resolve(frame, value)?;
                    let ty = ins.ty.unwrap_or(infer_ty(&value));
                    self.memory.store(ptr, ty, &value)?;
                }
                InstructionKind::Cmpxchg {
                    addr,
                    expected,
                    replacement,
                    ..
                } => {
                    let ptr = self.resolve_addr(frame, addr)?;
                    let ty = ins.ty.unwrap_or(PrimType::I64);
                    let old = self.memory.load(ptr, ty)?;
                    let expected = self.resolve(frame, expected)?;
                    let success = old == expected;
                    if success {
                        let replacement = self.resolve(frame, replacement)?;
                        self.memory.store(ptr, ty, &replacement)?;
                    }
                    set_dest(frame, ins, old);
                    set_extra_dests(frame, ins, Value::Int(i128::from(success)));
                }
                InstructionKind::AtomicRmw { op, addr, value, .. } => {
                    let ptr = self.resolve_addr(frame, addr)?;
                    let ty = ins.ty.unwrap_or(PrimType::I64);
                    let old = self.memory.load(ptr, ty)?;
                    let rhs = self.resolve(frame, value)?;
                    let new = apply_op(rmw_op(*op), &[old.clone(), rhs], Some(ty))?;
                    self.memory.store(ptr, ty, &new)?;
                    set_dest(frame, ins, old);
                }
                InstructionKind::Fence { .. }
                | InstructionKind::VaStart
                | InstructionKind::VaEnd
                | InstructionKind::Nop
                | InstructionKind::Native { .. } => {}
                InstructionKind::Try { src } => {
                    let value = self.resolve(frame, src)?;
                    let is_error = matches!(value, Value::Ptr(0) | Value::Int(0))
                        || matches!(&value, Value::Bytes(b) if b.first().copied().unwrap_or(0) != 0);
                    if is_error {
                        return Ok(Value::Int(0));
                    }
                    set_dest(frame, ins, value);
                }
                InstructionKind::Panic { message, args } => {
                    let mut text = message.clone().unwrap_or_default();
                    if !args.is_empty() {
                        let mut parts = Vec::new();
                        for arg in args {
                            parts.push(self.resolve(frame, arg)?.to_i128().to_string());
                        }
                        if text.is_empty() {
                            text = parts.join(" ");
                        } else {
                            for (i, part) in parts.iter().enumerate() {
                                text = text.replace(&format!("{{{i}}}"), part);
                            }
                        }
                    }
                    return Err(RuntimeError::new(
                        Trap::RuntimeError,
                        if text.is_empty() {
                            "panic".to_string()
                        } else {
                            text
                        },
                    ));
                }
                InstructionKind::VaArg { src } => {
                    let fallback = self.resolve(frame, src)?;
                    let value = frame
                        .va
                        .get(frame.va_pos)
                        .cloned()
                        .unwrap_or(fallback);
                    frame.va_pos += 1;
                    set_dest(frame, ins, value);
                }
            }
            if !matches!(ins.kind, InstructionKind::Cmpxchg { .. }) {
                let value = ins
                    .dest
                    .as_ref()
                    .and_then(|name| frame.regs.get(name))
                    .cloned()
                    .unwrap_or(Value::Int(0));
                set_extra_dests(frame, ins, value);
            }
        }
    }

    fn resolve(&self, frame: &Frame<'_>, op: &Operand) -> Result<Value, RuntimeError> {
        match op {
            Operand::Reg(name) => {
                if let Some(value) = frame.regs.get(name) {
                    return Ok(value.clone());
                }
                if let Some(c) = self.program.find_const(name) {
                    return Ok(const_value(&c.value));
                }
                if let Some(d) = self.program.find_def(name) {
                    return Ok(Value::Bytes(d.as_bytes().to_vec()));
                }
                Err(RuntimeError::new(
                    Trap::UnknownRegister,
                    format!("unknown register {name}"),
                ))
            }
            Operand::Int(v) => Ok(Value::Int(*v)),
            Operand::Float(v) => Ok(Value::Float(*v)),
            Operand::Bytes(b) => Ok(Value::Bytes(b.clone())),
            Operand::Utf8(s) => Ok(Value::Bytes(s.as_bytes().to_vec())),
            Operand::Symbol(name) => {
                if let Some(c) = self.program.find_const(name) {
                    Ok(const_value(&c.value))
                } else if let Some(d) = self.program.find_def(name) {
                    Ok(Value::Bytes(d.as_bytes().to_vec()))
                } else {
                    Ok(Value::Int(0))
                }
            }
            Operand::Label(_) | Operand::Undef => Ok(Value::Int(0)),
            Operand::Null => Ok(Value::Ptr(0)),
        }
    }

    fn resolve_arg(
        &mut self,
        frame: &Frame<'_>,
        arg: &sa_ir::CallArg,
    ) -> Result<Value, RuntimeError> {
        let value = self.resolve(frame, &arg.value)?;
        if arg.cap == CapPrefix::Borrow || arg.cap == CapPrefix::Raw {
            if let Value::Bytes(data) = &value {
                let ptr = self.memory.alloc(data.len().max(1), 1)?;
                self.memory.write(ptr, data)?;
                return Ok(Value::Ptr(ptr));
            }
        }
        Ok(value)
    }

    fn resolve_addr(&self, frame: &Frame<'_>, addr: &Address) -> Result<u64, RuntimeError> {
        let base = self.resolve(frame, &addr.base)?.as_u64().ok_or_else(|| {
            RuntimeError::new(Trap::RuntimeError, format!("invalid address {}", addr.base))
        })?;
        Ok(base.wrapping_add(addr.offset as u64))
    }

    fn jump(&self, frame: &mut Frame<'_>, label: &str) -> Result<(), RuntimeError> {
        let index = frame
            .func
            .blocks
            .iter()
            .position(|b| b.label == label || normalize_label(&b.label) == normalize_label(label))
            .ok_or_else(|| RuntimeError::new(Trap::RuntimeError, format!("unknown label {label}")))?;
        frame.block = index;
        frame.instr = 0;
        Ok(())
    }
}

fn normalize(name: &str) -> &str {
    name.trim().trim_start_matches('@')
}

fn normalize_label(name: &str) -> &str {
    name.trim().trim_start_matches("L_")
}

fn set_dest(frame: &mut Frame<'_>, ins: &Instruction, value: Value) {
    if let Some(dest) = &ins.dest {
        frame.regs.insert(dest.clone(), value);
    }
}

fn set_extra_dests(frame: &mut Frame<'_>, ins: &Instruction, value: Value) {
    for dest in &ins.extra_dests {
        frame.regs.insert(dest.clone(), value.clone());
    }
}

fn infer_ty(value: &Value) -> PrimType {
    match value {
        Value::Float(_) => PrimType::F64,
        Value::Ptr(_) => PrimType::Ptr,
        Value::Bytes(b) if b.len() >= 16 => PrimType::V128,
        Value::Bytes(_) | Value::Int(_) => PrimType::I64,
        Value::Void => PrimType::Void,
    }
}

fn rmw_op(op: AtomicRmwOp) -> OpKind {
    match op {
        AtomicRmwOp::Add => OpKind::Add,
        AtomicRmwOp::Sub => OpKind::Sub,
        AtomicRmwOp::And => OpKind::And,
        AtomicRmwOp::Or => OpKind::Or,
        AtomicRmwOp::Xor => OpKind::Xor,
        AtomicRmwOp::Xchg => OpKind::Bitcast,
        AtomicRmwOp::Umin => OpKind::Ult,
        AtomicRmwOp::Umax => OpKind::Ugt,
        AtomicRmwOp::Smin => OpKind::Slt,
        AtomicRmwOp::Smax => OpKind::Sgt,
    }
}

fn const_value(c: &ConstValue) -> Value {
    match c {
        ConstValue::Int(v) => Value::Int(*v),
        ConstValue::Float(v) => Value::Float(*v),
        ConstValue::Bool(v) => Value::Int(*v as i128),
        ConstValue::Bytes(b) => Value::Bytes(b.clone()),
        ConstValue::Utf8(s) => Value::Bytes(s.as_bytes().to_vec()),
        ConstValue::Null | ConstValue::Undef => Value::Ptr(0),
        ConstValue::Aggregate(v) | ConstValue::VTable(v) => {
            let mut out = Vec::new();
            for item in v {
                match const_value(item) {
                    Value::Bytes(b) => out.extend_from_slice(&b),
                    Value::Int(i) => out.extend_from_slice(&i.to_le_bytes()),
                    Value::Ptr(p) => out.extend_from_slice(&p.to_le_bytes()),
                    Value::Float(f) => out.extend_from_slice(&f.to_le_bytes()),
                    Value::Void => {}
                }
            }
            Value::Bytes(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{Block, CallArg, CapPrefix, Signature, SourceLoc};

    fn ins(kind: InstructionKind, dest: Option<&str>, ty: Option<PrimType>) -> Instruction {
        let mut i = Instruction::new(kind, SourceLoc::unknown(), "test");
        i.dest = dest.map(str::to_string);
        i.ty = ty;
        i
    }

    fn block(label: &str, instrs: Vec<Instruction>) -> Block {
        let mut b = Block::new(label, SourceLoc::unknown());
        b.instrs = instrs;
        b
    }

    fn func(name: &str, blocks: Vec<Block>) -> Function {
        let mut sig = Signature::new(name);
        sig.return_ty = PrimType::I64;
        let mut f = Function::new(sig, SourceLoc::unknown());
        f.blocks = blocks;
        f
    }

    fn program(functions: Vec<Function>) -> Program {
        Program {
            functions,
            ..Program::default()
        }
    }

    #[test]
    fn arithmetic_and_return() {
        let p = program(vec![func(
            "main",
            vec![block(
                "entry",
                vec![
                    ins(
                        InstructionKind::Op {
                            op: OpKind::Add,
                            args: vec![Operand::Int(2), Operand::Int(3)],
                        },
                        Some("r"),
                        None,
                    ),
                    ins(
                        InstructionKind::Return {
                            value: Some(Operand::reg("r")),
                        },
                        None,
                        None,
                    ),
                ],
            )],
        )]);
        let mut it = Interpreter::new(&p, InterpOptions::default());
        assert_eq!(it.run().unwrap(), 5);
        assert!(it.gas_used() > 0);
    }

    #[test]
    fn print_bytes_and_integers() {
        let p = program(vec![func(
            "main",
            vec![block(
                "entry",
                vec![
                    ins(InstructionKind::Alloc { size: Operand::Int(8) }, Some("p"), None),
                    ins(
                        InstructionKind::Store {
                            addr: Address::new(Operand::reg("p"), 0),
                            value: Operand::Utf8("hello".to_string()),
                        },
                        None,
                        None,
                    ),
                    ins(
                        InstructionKind::Call {
                            target: "sa_print_bytes".to_string(),
                            args: vec![
                                CallArg::new(CapPrefix::Borrow, Operand::reg("p")),
                                CallArg::new(CapPrefix::ByValue, Operand::Int(5)),
                            ],
                        },
                        None,
                        None,
                    ),
                    ins(
                        InstructionKind::Call {
                            target: "sa_print_i64".to_string(),
                            args: vec![CallArg::new(CapPrefix::ByValue, Operand::Int(42))],
                        },
                        None,
                        None,
                    ),
                    ins(InstructionKind::Return { value: None }, None, None),
                ],
            )],
        )]);
        let mut it = Interpreter::new(&p, InterpOptions::default());
        assert_eq!(it.run().unwrap(), 0);
        assert_eq!(it.last_output(), b"hello42");
    }

    #[test]
    fn branches_and_labels() {
        let p = program(vec![func(
            "main",
            vec![
                block(
                    "entry",
                    vec![ins(
                        InstructionKind::Br {
                            cond: Operand::Int(0),
                            then_label: "L_then".to_string(),
                            else_label: "L_else".to_string(),
                        },
                        None,
                        None,
                    )],
                ),
                block(
                    "L_then",
                    vec![ins(
                        InstructionKind::Return {
                            value: Some(Operand::Int(7)),
                        },
                        None,
                        None,
                    )],
                ),
                block(
                    "L_else",
                    vec![ins(
                        InstructionKind::Return {
                            value: Some(Operand::Int(9)),
                        },
                        None,
                        None,
                    )],
                ),
            ],
        )]);
        let mut it = Interpreter::new(&p, InterpOptions::default());
        assert_eq!(it.run().unwrap(), 9);
    }

    #[test]
    fn capability_ops_keep_values() {
        let p = program(vec![func(
            "main",
            vec![block(
                "entry",
                vec![
                    ins(
                        InstructionKind::Borrow {
                            src: Operand::Int(4),
                            mutable: true,
                        },
                        Some("a"),
                        None,
                    ),
                    ins(InstructionKind::Release { reg: "a".to_string() }, None, None),
                    ins(
                        InstructionKind::Move {
                            src: Operand::reg("a"),
                        },
                        Some("b"),
                        None,
                    ),
                    ins(
                        InstructionKind::Return {
                            value: Some(Operand::reg("b")),
                        },
                        None,
                        None,
                    ),
                ],
            )],
        )]);
        let mut it = Interpreter::new(&p, InterpOptions::default());
        assert_eq!(it.run().unwrap(), 4);
    }

    #[test]
    fn division_by_zero_traps() {
        let p = program(vec![func(
            "main",
            vec![block(
                "entry",
                vec![ins(
                    InstructionKind::Op {
                        op: OpKind::SDiv,
                        args: vec![Operand::Int(1), Operand::Int(0)],
                    },
                    Some("r"),
                    None,
                )],
            )],
        )]);
        let mut it = Interpreter::new(&p, InterpOptions::default());
        let e = it.run().unwrap_err();
        assert_eq!(e.trap, Trap::DivisionByZero);
    }

    #[test]
    fn gas_limit_traps() {
        let p = program(vec![func(
            "main",
            vec![block(
                "entry",
                vec![
                    ins(
                        InstructionKind::Op {
                            op: OpKind::Add,
                            args: vec![Operand::Int(1), Operand::Int(2)],
                        },
                        Some("r"),
                        None,
                    ),
                    ins(InstructionKind::Return { value: None }, None, None),
                ],
            )],
        )]);
        let opts = InterpOptions {
            gas_limit: 1,
            ..InterpOptions::default()
        };
        let mut it = Interpreter::new(&p, opts);
        assert_eq!(it.run().unwrap_err().trap, Trap::GasExceeded);
    }

    #[test]
    fn function_calls_and_unknown_extern() {
        let mut add = func(
            "add",
            vec![block(
                "entry",
                vec![ins(
                    InstructionKind::Op {
                        op: OpKind::Add,
                        args: vec![Operand::reg("x"), Operand::reg("y")],
                    },
                    Some("r"),
                    None,
                ),
                ins(
                    InstructionKind::Return {
                        value: Some(Operand::reg("r")),
                    },
                    None,
                    None,
                )],
            )],
        );
        add.sig
            .params
            .push(sa_ir::ParamSpec::new("x", PrimType::I64, CapPrefix::ByValue));
        add.sig
            .params
            .push(sa_ir::ParamSpec::new("y", PrimType::I64, CapPrefix::ByValue));
        let p = program(vec![func(
            "main",
            vec![block(
                "entry",
                vec![ins(
                    InstructionKind::Call {
                        target: "add".to_string(),
                        args: vec![
                            CallArg::new(CapPrefix::ByValue, Operand::Int(10)),
                            CallArg::new(CapPrefix::ByValue, Operand::Int(5)),
                        ],
                    },
                    Some("r"),
                    None,
                ),
                ins(
                    InstructionKind::Return {
                        value: Some(Operand::reg("r")),
                    },
                    None,
                    None,
                )],
            )],
        ), add]);
        let mut it = Interpreter::new(&p, InterpOptions::default());
        assert_eq!(it.run().unwrap(), 15);
        assert_eq!(
            it.run_function("@unknown", Vec::new()).unwrap(),
            Value::Int(0)
        );
    }

    #[test]
    fn cmpxchg_binds_extra_success_dest() {
        let mut exchange = ins(
            InstructionKind::Cmpxchg {
                addr: Address::new(Operand::reg("p"), 0),
                expected: Operand::Int(0),
                replacement: Operand::Int(1),
                order: sa_ir::AtomicOrdering::SeqCst,
            },
            Some("old"),
            Some(PrimType::I32),
        );
        exchange.extra_dests = vec!["ok".into()];
        let p = program(vec![func(
            "main",
            vec![block(
                "entry",
                vec![
                    ins(InstructionKind::Alloc { size: Operand::Int(8) }, Some("p"), None),
                    ins(
                        InstructionKind::Store {
                            addr: Address::new(Operand::reg("p"), 0),
                            value: Operand::Int(0),
                        },
                        None,
                        Some(PrimType::I32),
                    ),
                    exchange,
                    ins(
                        InstructionKind::Return {
                            value: Some(Operand::reg("ok")),
                        },
                        None,
                        None,
                    ),
                ],
            )],
        )]);
        let mut it = Interpreter::new(&p, InterpOptions::default());
        assert_eq!(it.run().unwrap(), 1);
    }

    #[test]
    fn trace_records_instructions() {
        let p = program(vec![func(
            "main",
            vec![block("entry", vec![ins(InstructionKind::Nop, None, None)])],
        )]);
        let opts = InterpOptions {
            trace: true,
            ..InterpOptions::default()
        };
        let mut it = Interpreter::new(&p, opts);
        it.run().unwrap();
        assert_eq!(it.trace_log().len(), 1);
    }
}
