use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimType {
    Void,
    I1,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Ptr,
    BlobHandle,
    V128,
}

impl PrimType {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "void" => Some(Self::Void),
            "i1" => Some(Self::I1),
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "ptr" => Some(Self::Ptr),
            "blob_handle" => Some(Self::BlobHandle),
            "v128" => Some(Self::V128),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::I1 => "i1",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Ptr => "ptr",
            Self::BlobHandle => "blob_handle",
            Self::V128 => "v128",
        }
    }

    pub fn bits(self) -> u32 {
        match self {
            Self::Void => 0,
            Self::I1 => 1,
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 | Self::F32 => 32,
            Self::I64 | Self::U64 | Self::F64 | Self::Ptr | Self::BlobHandle => 64,
            Self::V128 => 128,
        }
    }

    pub fn bytes(self) -> u32 {
        if self == Self::Void {
            0
        } else {
            (self.bits() / 8).max(1)
        }
    }

    pub fn is_void(self) -> bool {
        self == Self::Void
    }

    pub fn is_integer(self) -> bool {
        matches!(
            self,
            Self::I1
                | Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
        )
    }

    pub fn is_signed(self) -> bool {
        matches!(
            self,
            Self::I1 | Self::I8 | Self::I16 | Self::I32 | Self::I64
        )
    }

    pub fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    pub fn is_pointer(self) -> bool {
        matches!(self, Self::Ptr | Self::BlobHandle)
    }
}

impl fmt::Display for PrimType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapPrefix {
    ByValue,
    Borrow,
    Move,
    Raw,
}

impl CapPrefix {
    pub fn parse(text: &str) -> Option<(Self, &str)> {
        match text.as_bytes().first() {
            Some(b'&') => Some((Self::Borrow, &text[1..])),
            Some(b'^') => Some((Self::Move, &text[1..])),
            Some(b'*') => Some((Self::Raw, &text[1..])),
            _ => None,
        }
    }

    pub fn prefix(self) -> &'static str {
        match self {
            Self::ByValue => "",
            Self::Borrow => "&",
            Self::Move => "^",
            Self::Raw => "*",
        }
    }
}

impl fmt::Display for CapPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.prefix())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParamSpec {
    pub name: String,
    pub ty: PrimType,
    pub cap: CapPrefix,
}

impl ParamSpec {
    pub fn new(name: impl Into<String>, ty: PrimType, cap: CapPrefix) -> Self {
        Self {
            name: name.into(),
            ty,
            cap,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionKind {
    Normal,
    FfiWrapper,
    External,
    Exported,
    TestFunc,
    Gpu,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    pub id: u32,
    pub name: String,
    pub params: Vec<ParamSpec>,
    pub kind: FunctionKind,
    pub return_cap: Option<CapPrefix>,
    pub return_ty: PrimType,
    pub return_fallible: bool,
    pub is_variadic: bool,
}

impl Signature {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: 0,
            name: name.into(),
            params: Vec::new(),
            kind: FunctionKind::Normal,
            return_cap: None,
            return_ty: PrimType::Void,
            return_fallible: false,
            is_variadic: false,
        }
    }

    pub fn display_name(&self) -> &str {
        if self.kind == FunctionKind::TestFunc
            && self.name.len() >= 2
            && self.name.starts_with('"')
            && self.name.ends_with('"')
        {
            &self.name[1..self.name.len() - 1]
        } else {
            &self.name
        }
    }

    pub fn return_value_type(&self) -> PrimType {
        if self.return_ty.is_void() {
            return PrimType::Void;
        }
        match self.return_cap.unwrap_or(CapPrefix::ByValue) {
            CapPrefix::Raw | CapPrefix::Borrow => PrimType::Ptr,
            CapPrefix::Move | CapPrefix::ByValue => self.return_ty,
        }
    }

    pub fn header_text(&self) -> String {
        let mut out = String::new();
        match self.kind {
            FunctionKind::External => out.push_str("@extern "),
            FunctionKind::FfiWrapper => out.push_str("@ffi_wrapper "),
            FunctionKind::Exported => out.push_str("@export "),
            FunctionKind::TestFunc => out.push_str("@test "),
            FunctionKind::Gpu => out.push_str("@gpu "),
            FunctionKind::Normal => out.push('@'),
        }
        out.push_str(&self.name);
        out.push('(');
        for (i, p) in self.params.iter().enumerate() {
            if i != 0 {
                out.push_str(", ");
            }
            out.push_str(p.cap.prefix());
            out.push_str(&p.name);
            out.push_str(": ");
            out.push_str(p.ty.name());
        }
        if self.is_variadic {
            if !self.params.is_empty() {
                out.push_str(", ");
            }
            out.push_str("...");
        }
        out.push(')');
        if !self.return_ty.is_void() || self.return_fallible {
            out.push_str(" -> ");
            if let Some(cap) = self.return_cap {
                out.push_str(cap.prefix());
            }
            out.push_str(self.return_ty.name());
            if self.return_fallible {
                out.push('!');
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicOrdering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

impl AtomicOrdering {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "relaxed" => Some(Self::Relaxed),
            "acquire" => Some(Self::Acquire),
            "release" => Some(Self::Release),
            "acq_rel" => Some(Self::AcqRel),
            "seq_cst" => Some(Self::SeqCst),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Relaxed => "relaxed",
            Self::Acquire => "acquire",
            Self::Release => "release",
            Self::AcqRel => "acq_rel",
            Self::SeqCst => "seq_cst",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicRmwOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Xchg,
    Umin,
    Umax,
    Smin,
    Smax,
}

impl AtomicRmwOp {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "add" => Some(Self::Add),
            "sub" => Some(Self::Sub),
            "and" => Some(Self::And),
            "or" => Some(Self::Or),
            "xor" => Some(Self::Xor),
            "xchg" => Some(Self::Xchg),
            "umin" => Some(Self::Umin),
            "umax" => Some(Self::Umax),
            "smin" => Some(Self::Smin),
            "smax" => Some(Self::Smax),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::And => "and",
            Self::Or => "or",
            Self::Xor => "xor",
            Self::Xchg => "xchg",
            Self::Umin => "umin",
            Self::Umax => "umax",
            Self::Smin => "smin",
            Self::Smax => "smax",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Int(i128),
    Float(f64),
    Bool(bool),
    Bytes(Vec<u8>),
    Utf8(String),
    Null,
    Undef,
    Aggregate(Vec<ConstValue>),
    VTable(Vec<ConstValue>),
}

impl ConstValue {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => i64::try_from(*v).ok(),
            Self::Bool(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn bytes(&self) -> Option<Vec<u8>> {
        match self {
            Self::Bytes(b) => Some(b.clone()),
            Self::Utf8(s) => Some(s.as_bytes().to_vec()),
            _ => None,
        }
    }

    pub fn is_const(&self) -> bool {
        true
    }
}

impl fmt::Display for ConstValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Bytes(b) => write!(f, "bytes:{}", b.len()),
            Self::Utf8(s) => write!(f, "utf8:\"{s}\""),
            Self::Null => f.write_str("null"),
            Self::Undef => f.write_str("undef"),
            Self::Aggregate(v) => write!(f, "agg:{}", v.len()),
            Self::VTable(v) => write!(f, "vtable:{}", v.len()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name: String,
    pub value: ConstValue,
    pub ty: Option<PrimType>,
}

impl ConstDecl {
    pub fn new(name: impl Into<String>, value: ConstValue) -> Self {
        Self {
            name: name.into(),
            value,
            ty: None,
        }
    }
}
