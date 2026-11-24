use crate::instr::Instruction;
use crate::types::{ConstDecl, Signature};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SourceLoc {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl SourceLoc {
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }

    pub fn unknown() -> Self {
        Self::default()
    }

    pub fn is_unknown(&self) -> bool {
        self.file.is_empty() && self.line == 0
    }
}

impl fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.file.is_empty() {
            write!(f, "{}:{}", self.line, self.column)
        } else {
            write!(f, "{}:{}:{}", self.file, self.line, self.column)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub path: String,
    pub alias: Option<String>,
    pub loc: SourceLoc,
}

impl ImportDecl {
    pub fn new(path: impl Into<String>, loc: SourceLoc) -> Self {
        Self {
            path: path.into(),
            alias: None,
            loc,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DefDecl {
    pub name: String,
    pub value: String,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocDecl {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub original_line: u32,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub label: String,
    pub instrs: Vec<Instruction>,
    pub loc: SourceLoc,
}

impl Block {
    pub fn new(label: impl Into<String>, loc: SourceLoc) -> Self {
        Self {
            label: label.into(),
            instrs: Vec::new(),
            loc,
        }
    }

    pub fn to_text(&self, indent: &str) -> String {
        let mut out = format!("{}{}:\n", indent, self.label);
        for ins in &self.instrs {
            out.push_str(indent);
            out.push_str("    ");
            out.push_str(&ins.to_text());
            out.push('\n');
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub sig: Signature,
    pub blocks: Vec<Block>,
    pub locals: Vec<String>,
    pub loc: SourceLoc,
}

impl Function {
    pub fn new(sig: Signature, loc: SourceLoc) -> Self {
        Self {
            sig,
            blocks: Vec::new(),
            locals: Vec::new(),
            loc,
        }
    }

    pub fn name(&self) -> &str {
        &self.sig.name
    }

    pub fn entry(&self) -> Option<&Block> {
        self.blocks.first()
    }

    pub fn labels(&self) -> Vec<&str> {
        self.blocks.iter().map(|b| b.label.as_str()).collect()
    }

    pub fn all_instructions(&self) -> Vec<&Instruction> {
        let mut out = Vec::new();
        for block in &self.blocks {
            for ins in &block.instrs {
                out.push(ins);
            }
        }
        out
    }

    pub fn to_text(&self) -> String {
        let mut out = format!("{}:\n", self.sig.header_text());
        for block in &self.blocks {
            out.push_str(&block.to_text(""));
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    pub imports: Vec<ImportDecl>,
    pub defs: Vec<DefDecl>,
    pub consts: Vec<ConstDecl>,
    pub externs: Vec<Signature>,
    pub functions: Vec<Function>,
    pub locs: Vec<LocDecl>,
}

impl Program {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.sig.name == name)
    }

    pub fn function_mut(&mut self, name: &str) -> Option<&mut Function> {
        self.functions.iter_mut().find(|f| f.sig.name == name)
    }

    pub fn find_const(&self, name: &str) -> Option<&ConstDecl> {
        self.consts.iter().find(|c| c.name == name)
    }

    pub fn find_def(&self, name: &str) -> Option<&str> {
        self.defs
            .iter()
            .find(|d| d.name == name)
            .map(|d| d.value.as_str())
    }

    pub fn has_extern(&self, name: &str) -> bool {
        self.externs.iter().any(|e| e.name == name)
    }

    pub fn extern_sig(&self, name: &str) -> Option<&Signature> {
        self.externs.iter().find(|e| e.name == name)
    }

    pub fn entry_function(&self) -> Option<&Function> {
        self.functions
            .iter()
            .find(|f| f.sig.name == "main")
            .or_else(|| {
                self.functions
                    .iter()
                    .find(|f| f.sig.kind == crate::types::FunctionKind::Exported)
            })
            .or_else(|| self.functions.first())
    }

    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    pub fn instruction_count(&self) -> usize {
        self.functions
            .iter()
            .map(|f| f.all_instructions().len())
            .sum()
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for import in &self.imports {
            out.push_str(&format!("@import \"{}\"\n", import.path));
        }
        for def in &self.defs {
            out.push_str(&format!("#def {} = {}\n", def.name, def.value));
        }
        for c in &self.consts {
            out.push_str(&format!(
                "@const {} = {}\n",
                c.name,
                c.value
                    .ty_hint(c.ty)
            ));
        }
        for e in &self.externs {
            out.push_str(&e.header_text());
            out.push('\n');
        }
        for f in &self.functions {
            out.push_str(&f.to_text());
            out.push('\n');
        }
        out
    }
}

trait TyHint {
    fn ty_hint(&self, ty: Option<crate::types::PrimType>) -> String;
}

impl TyHint for crate::types::ConstValue {
    fn ty_hint(&self, ty: Option<crate::types::PrimType>) -> String {
        let base = self.to_string();
        match ty {
            Some(t) => format!("{base} as {t}"),
            None => base,
        }
    }
}
