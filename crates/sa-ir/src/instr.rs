use crate::program::SourceLoc;
use crate::types::{AtomicOrdering, AtomicRmwOp, CapPrefix, PrimType};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpKind {
    Add,
    Sub,
    Mul,
    SDiv,
    UDiv,
    SRem,
    URem,
    Neg,
    And,
    Or,
    Xor,
    Shl,
    LShr,
    AShr,
    Not,
    Eq,
    Ne,
    Slt,
    Sle,
    Sgt,
    Sge,
    Ult,
    Ule,
    Ugt,
    Uge,
    Div,
    Rem,
    Gt,
    Lt,
    Shr,
    FAdd,
    FSub,
    FMul,
    FDiv,
    FNeg,
    FcmpEq,
    FcmpNe,
    FcmpLt,
    FcmpLe,
    FcmpGt,
    FcmpGe,
    Trunc,
    Zext,
    Sext,
    Fptosi,
    Sitofp,
    Uitofp,
    Fptrunc,
    Fpext,
    Bitcast,
    AddV128,
    SubV128,
    MulV128,
    ShuffleV128,
    ExtractLane,
    InsertLane,
}

impl OpKind {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "add" => Some(Self::Add),
            "sub" => Some(Self::Sub),
            "mul" => Some(Self::Mul),
            "sdiv" => Some(Self::SDiv),
            "udiv" => Some(Self::UDiv),
            "srem" => Some(Self::SRem),
            "urem" => Some(Self::URem),
            "neg" => Some(Self::Neg),
            "and" => Some(Self::And),
            "or" => Some(Self::Or),
            "xor" => Some(Self::Xor),
            "shl" => Some(Self::Shl),
            "lshr" => Some(Self::LShr),
            "ashr" => Some(Self::AShr),
            "not" => Some(Self::Not),
            "eq" => Some(Self::Eq),
            "ne" => Some(Self::Ne),
            "slt" => Some(Self::Slt),
            "sle" => Some(Self::Sle),
            "sgt" => Some(Self::Sgt),
            "sge" => Some(Self::Sge),
            "ult" => Some(Self::Ult),
            "ule" => Some(Self::Ule),
            "ugt" => Some(Self::Ugt),
            "uge" => Some(Self::Uge),
            "div" => Some(Self::Div),
            "rem" => Some(Self::Rem),
            "gt" => Some(Self::Gt),
            "lt" => Some(Self::Lt),
            "shr" => Some(Self::Shr),
            "fadd" => Some(Self::FAdd),
            "fsub" => Some(Self::FSub),
            "fmul" => Some(Self::FMul),
            "fdiv" => Some(Self::FDiv),
            "fneg" => Some(Self::FNeg),
            "fcmp_eq" => Some(Self::FcmpEq),
            "fcmp_ne" => Some(Self::FcmpNe),
            "fcmp_lt" => Some(Self::FcmpLt),
            "fcmp_le" => Some(Self::FcmpLe),
            "fcmp_gt" => Some(Self::FcmpGt),
            "fcmp_ge" => Some(Self::FcmpGe),
            "trunc" => Some(Self::Trunc),
            "zext" => Some(Self::Zext),
            "sext" => Some(Self::Sext),
            "fptosi" => Some(Self::Fptosi),
            "sitofp" => Some(Self::Sitofp),
            "uitofp" => Some(Self::Uitofp),
            "fptrunc" => Some(Self::Fptrunc),
            "fpext" => Some(Self::Fpext),
            "bitcast" => Some(Self::Bitcast),
            "add_v128" => Some(Self::AddV128),
            "sub_v128" => Some(Self::SubV128),
            "mul_v128" => Some(Self::MulV128),
            "shuffle_v128" => Some(Self::ShuffleV128),
            "extract_lane" => Some(Self::ExtractLane),
            "insert_lane" => Some(Self::InsertLane),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::SDiv => "sdiv",
            Self::UDiv => "udiv",
            Self::SRem => "srem",
            Self::URem => "urem",
            Self::Neg => "neg",
            Self::And => "and",
            Self::Or => "or",
            Self::Xor => "xor",
            Self::Shl => "shl",
            Self::LShr => "lshr",
            Self::AShr => "ashr",
            Self::Not => "not",
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Slt => "slt",
            Self::Sle => "sle",
            Self::Sgt => "sgt",
            Self::Sge => "sge",
            Self::Ult => "ult",
            Self::Ule => "ule",
            Self::Ugt => "ugt",
            Self::Uge => "uge",
            Self::Div => "div",
            Self::Rem => "rem",
            Self::Gt => "gt",
            Self::Lt => "lt",
            Self::Shr => "shr",
            Self::FAdd => "fadd",
            Self::FSub => "fsub",
            Self::FMul => "fmul",
            Self::FDiv => "fdiv",
            Self::FNeg => "fneg",
            Self::FcmpEq => "fcmp_eq",
            Self::FcmpNe => "fcmp_ne",
            Self::FcmpLt => "fcmp_lt",
            Self::FcmpLe => "fcmp_le",
            Self::FcmpGt => "fcmp_gt",
            Self::FcmpGe => "fcmp_ge",
            Self::Trunc => "trunc",
            Self::Zext => "zext",
            Self::Sext => "sext",
            Self::Fptosi => "fptosi",
            Self::Sitofp => "sitofp",
            Self::Uitofp => "uitofp",
            Self::Fptrunc => "fptrunc",
            Self::Fpext => "fpext",
            Self::Bitcast => "bitcast",
            Self::AddV128 => "add_v128",
            Self::SubV128 => "sub_v128",
            Self::MulV128 => "mul_v128",
            Self::ShuffleV128 => "shuffle_v128",
            Self::ExtractLane => "extract_lane",
            Self::InsertLane => "insert_lane",
        }
    }

    pub fn arity(self) -> usize {
        if self.is_unary() {
            1
        } else if self.is_ternary() {
            3
        } else {
            2
        }
    }

    pub fn is_unary(self) -> bool {
        matches!(
            self,
            Self::Neg
                | Self::Not
                | Self::FNeg
                | Self::Trunc
                | Self::Zext
                | Self::Sext
                | Self::Fptosi
                | Self::Sitofp
                | Self::Uitofp
                | Self::Fptrunc
                | Self::Fpext
                | Self::Bitcast
        )
    }

    pub fn is_ternary(self) -> bool {
        matches!(self, Self::ShuffleV128 | Self::InsertLane)
    }

    pub fn is_binary(self) -> bool {
        !self.is_unary() && !self.is_ternary()
    }

    pub fn is_conversion(self) -> bool {
        matches!(
            self,
            Self::Trunc
                | Self::Zext
                | Self::Sext
                | Self::Fptosi
                | Self::Sitofp
                | Self::Uitofp
                | Self::Fptrunc
                | Self::Fpext
                | Self::Bitcast
        )
    }

    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq
                | Self::Ne
                | Self::Slt
                | Self::Sle
                | Self::Sgt
                | Self::Sge
                | Self::Ult
                | Self::Ule
                | Self::Ugt
                | Self::Uge
                | Self::Gt
                | Self::Lt
                | Self::FcmpEq
                | Self::FcmpNe
                | Self::FcmpLt
                | Self::FcmpLe
                | Self::FcmpGt
                | Self::FcmpGe
        )
    }
}

impl fmt::Display for OpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Reg(String),
    Int(i128),
    Float(f64),
    Bytes(Vec<u8>),
    Utf8(String),
    Symbol(String),
    Label(String),
    Null,
    Undef,
}

impl Operand {
    pub fn reg(name: impl Into<String>) -> Self {
        Self::Reg(name.into())
    }

    pub fn parse(text: &str) -> Self {
        let mut t = text.trim();
        while let Some(rest) = t.strip_prefix('&').or_else(|| t.strip_prefix('^')).or_else(|| t.strip_prefix('*')) {
            t = rest.trim();
        }
        if let Some(rest) = t.strip_prefix("utf8:\"") {
            let body = rest.strip_suffix('"').unwrap_or(rest);
            return Self::Utf8(unescape(body));
        }
        if let Some(rest) = t.strip_prefix("bytes:") {
            return Self::Bytes(rest.as_bytes().to_vec());
        }
        if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
            return Self::Utf8(unescape(&t[1..t.len() - 1]));
        }
        if let Some(rest) = t.strip_prefix('@') {
            return Self::Symbol(rest.to_string());
        }
        if t.starts_with("L_") && t.len() > 2 {
            return Self::Label(t.to_string());
        }
        match t {
            "null" => return Self::Null,
            "undef" | "?" => return Self::Undef,
            _ => {}
        }
        let cleaned = t.replace('_', "");
        if let Some(hex) = cleaned.strip_prefix("0x").or_else(|| cleaned.strip_prefix("0X")) {
            if let Ok(v) = i128::from_str_radix(hex, 16) {
                return Self::Int(v);
            }
        }
        if let Ok(v) = cleaned.parse::<i128>() {
            return Self::Int(v);
        }
        if let Ok(v) = t.parse::<f64>() {
            return Self::Float(v);
        }
        Self::Reg(t.to_string())
    }

    pub fn as_reg(&self) -> Option<&str> {
        match self {
            Self::Reg(name) => Some(name),
            _ => None,
        }
    }

    pub fn as_label(&self) -> Option<&str> {
        match self {
            Self::Label(name) => Some(name),
            Self::Reg(name) if name.starts_with("L_") => Some(name),
            _ => None,
        }
    }

    pub fn as_symbol(&self) -> Option<&str> {
        match self {
            Self::Symbol(name) => Some(name),
            _ => None,
        }
    }

    pub fn is_const(&self) -> bool {
        !matches!(self, Self::Reg(_) | Self::Label(_))
    }

    pub fn const_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => i64::try_from(*v).ok(),
            _ => None,
        }
    }

    pub fn const_f64(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v) => Some(*v as f64),
            _ => None,
        }
    }
}

/// Decode backslash escapes into raw bytes with sci-compatible strict rules.
///
/// Mirrors sci `decodeQuotedBytes` (`src/interp/memory.zig:285-336`):
/// `\\ \" \n \r \t \0` map to their byte values, `\xHH` maps to one byte,
/// anything else (including a trailing backslash or an incomplete `\x`) is an
/// error. Raw characters are appended as their UTF-8 encoding.
pub fn unescape_bytes(body: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        let next = chars
            .next()
            .ok_or_else(|| "trailing backslash in string literal".to_string())?;
        match next {
            'n' => out.push(b'\n'),
            'r' => out.push(b'\r'),
            't' => out.push(b'\t'),
            '0' => out.push(0),
            '"' => out.push(b'"'),
            '\\' => out.push(b'\\'),
            'x' => {
                let hi = chars
                    .next()
                    .and_then(|c| c.to_digit(16))
                    .ok_or_else(|| "invalid \\x escape: expected hex digits".to_string())?;
                let lo = chars
                    .next()
                    .and_then(|c| c.to_digit(16))
                    .ok_or_else(|| "invalid \\x escape: expected hex digits".to_string())?;
                out.push(((hi << 4) | lo) as u8);
            }
            other => return Err(format!("unknown escape sequence \\{}", other)),
        }
    }
    Ok(out)
}

/// Decode a fully quoted literal (leading and trailing `"`) into raw bytes.
pub fn decode_quoted(raw: &str) -> Result<Vec<u8>, String> {
    if raw.len() < 2 || !raw.starts_with('"') || !raw.ends_with('"') {
        return Err("string literal must start and end with '\"'".to_string());
    }
    unescape_bytes(&raw[1..raw.len() - 1])
}

/// Whether `bytes` is a well-formed UTF-8 sequence (sci `utf8ValidateSlice`).
pub fn utf8_valid(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
}

/// Lenient decoder used by `Operand::parse` on the live path.
///
/// Keeps the historical behaviour for unknown escapes / non-UTF-8 byte escapes
/// so the 968-name corpus stays byte-identical, but now recognises `\xHH`
/// (previously preserved literally) when it forms valid UTF-8.
fn unescape(s: &str) -> String {
    if let Ok(bytes) = unescape_bytes(s) {
        if let Ok(text) = String::from_utf8(bytes) {
            return text;
        }
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('0') => out.push('\0'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reg(s) | Self::Label(s) => f.write_str(s),
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Bytes(b) => write!(f, "bytes:{}", b.len()),
            Self::Utf8(s) => write!(f, "utf8:\"{}\"", escape(s)),
            Self::Symbol(s) => write!(f, "@{s}"),
            Self::Null => f.write_str("null"),
            Self::Undef => f.write_str("undef"),
        }
    }
}

fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Address {
    pub base: Operand,
    pub offset: i64,
}

impl Address {
    pub fn new(base: Operand, offset: i64) -> Self {
        Self { base, offset }
    }

    pub fn parse(text: &str) -> Self {
        let t = text.trim();
        if let Some(pos) = t.find('+') {
            let base = Operand::parse(&t[..pos]);
            let off = t[pos + 1..].trim().parse::<i64>().unwrap_or(0);
            return Self::new(base, off);
        }
        if let Some(pos) = t.rfind('-') {
            if pos > 0 {
                let base = Operand::parse(&t[..pos]);
                let off = t[pos + 1..].trim().parse::<i64>().unwrap_or(0);
                return Self::new(base, -off);
            }
        }
        Self::new(Operand::parse(t), 0)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.offset < 0 {
            write!(f, "{}{}", self.base, self.offset)
        } else {
            write!(f, "{}+{}", self.base, self.offset)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallArg {
    pub cap: CapPrefix,
    pub value: Operand,
}

impl CallArg {
    pub fn new(cap: CapPrefix, value: Operand) -> Self {
        Self { cap, value }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstructionKind {
    Alloc { size: Operand },
    StackAlloc { size: Operand },
    Load { addr: Address },
    Store { addr: Address, value: Operand },
    Op { op: OpKind, args: Vec<Operand> },
    Jmp { target: String },
    Br { cond: Operand, then_label: String, else_label: String },
    BrNull { cond: Operand, target: String, else_target: Option<String> },
    Call { target: String, args: Vec<CallArg> },
    CallIndirect { target: Operand, args: Vec<CallArg> },
    Return { value: Option<Operand> },
    Take { addr: Address },
    Release { reg: String },
    Assign { src: Operand },
    Move { src: Operand },
    PtrAdd { base: Operand, offset: Operand },
    Borrow { src: Operand, mutable: bool },
    RawCast { src: Operand },
    AssumeSafe { src: Operand },
    AssumeBorrow { src: Operand, mutable: bool },
    AtomicLoad { addr: Address, order: AtomicOrdering },
    AtomicStore { addr: Address, value: Operand, order: AtomicOrdering },
    Cmpxchg { addr: Address, expected: Operand, replacement: Operand, order: AtomicOrdering },
    AtomicRmw { op: AtomicRmwOp, addr: Address, value: Operand, order: AtomicOrdering },
    Fence { order: AtomicOrdering },
    Try { src: Operand },
    EarlyReturn { value: Option<Operand> },
    Panic { message: Option<String>, args: Vec<Operand> },
    VaStart,
    VaArg { src: Operand },
    VaEnd,
    Native { text: String },
    Nop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instruction {
    pub kind: InstructionKind,
    pub dest: Option<String>,
    pub extra_dests: Vec<String>,
    pub ty: Option<PrimType>,
    pub loc: SourceLoc,
    pub raw: String,
}

impl Instruction {
    pub fn new(kind: InstructionKind, loc: SourceLoc, raw: impl Into<String>) -> Self {
        Self {
            kind,
            dest: None,
            extra_dests: Vec::new(),
            ty: None,
            loc,
            raw: raw.into(),
        }
    }

    pub fn with_dest(mut self, dest: impl Into<String>) -> Self {
        self.dest = Some(dest.into());
        self
    }

    pub fn with_extra_dests<I, S>(mut self, dests: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.extra_dests = dests.into_iter().map(Into::into).collect();
        self
    }

    pub fn all_dests(&self) -> impl Iterator<Item = &str> {
        self.dest
            .as_deref()
            .into_iter()
            .chain(self.extra_dests.iter().map(String::as_str))
    }

    pub fn with_ty(mut self, ty: PrimType) -> Self {
        self.ty = Some(ty);
        self
    }

    pub fn write_reg(&self) -> Option<&str> {
        self.dest.as_deref()
    }

    pub fn reads(&self) -> Vec<&Operand> {
        let mut out = Vec::new();
        match &self.kind {
            InstructionKind::Alloc { size } | InstructionKind::StackAlloc { size } => out.push(size),
            InstructionKind::Load { addr } => out.push(&addr.base),
            InstructionKind::Store { addr, value } => {
                out.push(&addr.base);
                out.push(value);
            }
            InstructionKind::Op { args, .. } => out.extend(args.iter()),
            InstructionKind::Br { cond, .. } | InstructionKind::BrNull { cond, .. } => {
                out.push(cond)
            }
            InstructionKind::Call { args, .. } | InstructionKind::CallIndirect { args, .. } => {
                for a in args {
                    out.push(&a.value);
                }
            }
            InstructionKind::Return { value } | InstructionKind::EarlyReturn { value } => {
                if let Some(v) = value {
                    out.push(v);
                }
            }
            InstructionKind::Assign { src }
            | InstructionKind::Move { src }
            | InstructionKind::RawCast { src }
            | InstructionKind::AssumeSafe { src }
            | InstructionKind::Try { src }
            | InstructionKind::VaArg { src } => out.push(src),
            InstructionKind::PtrAdd { base, offset } => {
                out.push(base);
                out.push(offset);
            }
            InstructionKind::Borrow { src, .. } | InstructionKind::AssumeBorrow { src, .. } => {
                out.push(src)
            }
            InstructionKind::AtomicLoad { addr, .. } => out.push(&addr.base),
            InstructionKind::AtomicStore { addr, value, .. } => {
                out.push(&addr.base);
                out.push(value);
            }
            InstructionKind::Cmpxchg { addr, expected, replacement, .. } => {
                out.push(&addr.base);
                out.push(expected);
                out.push(replacement);
            }
            InstructionKind::AtomicRmw { addr, value, .. } => {
                out.push(&addr.base);
                out.push(value);
            }
            InstructionKind::Panic { args, .. } => out.extend(args.iter()),
            InstructionKind::Take { addr } => out.push(&addr.base),
            InstructionKind::Release { .. }
            | InstructionKind::Jmp { .. }
            | InstructionKind::Fence { .. }
            | InstructionKind::VaStart
            | InstructionKind::VaEnd
            | InstructionKind::Native { .. }
            | InstructionKind::Nop => {}
        }
        out
    }

    pub fn gas_cost(&self) -> u64 {
        match self.kind {
            InstructionKind::Call { .. } | InstructionKind::CallIndirect { .. } => 4,
            InstructionKind::Alloc { .. } | InstructionKind::StackAlloc { .. } => 3,
            InstructionKind::AtomicRmw { .. } | InstructionKind::Cmpxchg { .. } => 3,
            InstructionKind::Jmp { .. }
            | InstructionKind::Br { .. }
            | InstructionKind::BrNull { .. }
            | InstructionKind::Return { .. } => 2,
            InstructionKind::Nop | InstructionKind::Native { .. } => 0,
            _ => 1,
        }
    }

    pub fn to_text(&self) -> String {
        let prefix = {
            let dests: Vec<&str> = self.all_dests().collect();
            if dests.is_empty() {
                String::new()
            } else {
                format!("{} = ", dests.join(", "))
            }
        };
        let body = match &self.kind {
            InstructionKind::Alloc { size } => format!("alloc {size}"),
            InstructionKind::StackAlloc { size } => format!("stack_alloc {size}"),
            InstructionKind::Load { addr } => format!("load {addr}"),
            InstructionKind::Store { addr, value } => format!("store {addr}, {value}"),
            InstructionKind::Op { op, args } => {
                let a: Vec<String> = args.iter().map(|x| x.to_string()).collect();
                format!("{} {}", op, a.join(", "))
            }
            InstructionKind::Jmp { target } => format!("jmp {target}"),
            InstructionKind::Br { cond, then_label, else_label } => {
                format!("br {cond} -> {then_label}, {else_label}")
            }
            InstructionKind::BrNull { cond, target, else_target } => match else_target {
                Some(other) => format!("br_null {cond} -> {target}, {other}"),
                None => format!("br_null {cond}, {target}"),
            },
            InstructionKind::Call { target, args } => {
                let a: Vec<String> = args
                    .iter()
                    .map(|x| format!("{}{}", x.cap, x.value))
                    .collect();
                format!("call @{target}({})", a.join(", "))
            }
            InstructionKind::CallIndirect { target, args } => {
                let a: Vec<String> = args
                    .iter()
                    .map(|x| format!("{}{}", x.cap, x.value))
                    .collect();
                format!("call_indirect {target}({})", a.join(", "))
            }
            InstructionKind::Return { value } => match value {
                Some(v) => format!("return {v}"),
                None => "return".to_string(),
            },
            InstructionKind::Take { addr } => format!("take {addr}"),
            InstructionKind::Release { reg } => format!("!{reg}"),
            InstructionKind::Assign { src } => src.to_string(),
            InstructionKind::Move { src } => format!("^{src}"),
            InstructionKind::PtrAdd { base, offset } => format!("ptr_add {base}, {offset}"),
            InstructionKind::Borrow { src, mutable } => {
                format!("&{}{}", if *mutable { "mut " } else { "" }, src)
            }
            InstructionKind::RawCast { src } => format!("*{src}"),
            InstructionKind::AssumeSafe { src } => format!("assume_safe {src}"),
            InstructionKind::AssumeBorrow { src, mutable } => format!(
                "assume_borrow {src}{}",
                if *mutable { ", mut" } else { "" }
            ),
            InstructionKind::AtomicLoad { addr, order } => {
                format!("atomic_load {addr} {order}", order = order.name())
            }
            InstructionKind::AtomicStore { addr, value, order } => format!(
                "atomic_store {addr}, {value} {order}",
                order = order.name()
            ),
            InstructionKind::Cmpxchg { addr, expected, replacement, order } => format!(
                "cmpxchg {addr}, {expected}, {replacement} {order}",
                order = order.name()
            ),
            InstructionKind::AtomicRmw { op, addr, value, order } => format!(
                "atomic_rmw {} {addr}, {value} {order}",
                op.name(),
                order = order.name()
            ),
            InstructionKind::Fence { order } => format!("fence {}", order.name()),
            InstructionKind::Try { src } => format!("?{src}"),
            InstructionKind::EarlyReturn { value } => match value {
                Some(v) => format!("early_return {v}"),
                None => "early_return".to_string(),
            },
            InstructionKind::Panic { message, args } => {
                if args.is_empty() {
                    format!("panic({})", message.clone().unwrap_or_default())
                } else {
                    format!("panic_msg({})", args.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))
                }
            }
            InstructionKind::VaStart => "va_start".to_string(),
            InstructionKind::VaArg { src } => format!("va_arg {src}"),
            InstructionKind::VaEnd => "va_end".to_string(),
            InstructionKind::Native { text } => text.clone(),
            InstructionKind::Nop => "nop".to_string(),
        };
        let mut line = format!("{prefix}{body}");
        if let Some(ty) = self.ty {
            line.push_str(&format!(" as {ty}"));
        }
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_dests_default_empty_and_render() {
        let loc = SourceLoc::unknown();
        let ins = Instruction::new(InstructionKind::Nop, loc.clone(), "nop");
        assert!(ins.extra_dests.is_empty());
        assert_eq!(ins.all_dests().count(), 0);
        assert_eq!(ins.to_text(), "nop");

        let ins = Instruction::new(InstructionKind::Nop, loc, "old, ok = nop")
            .with_dest("old")
            .with_extra_dests(["ok"])
            .with_ty(PrimType::I32);
        assert_eq!(ins.extra_dests, vec!["ok".to_string()]);
        assert_eq!(ins.all_dests().collect::<Vec<_>>(), vec!["old", "ok"]);
        assert_eq!(ins.to_text(), "old, ok = nop as i32");
    }
}
