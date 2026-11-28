use crate::parse_header;
use crate::parse_instr;
use sa_ir::{Block, Diagnostic, Function, FunctionKind, Program, SourceLoc, Trap};
use sa_lexer::{tokenize, Token, TokenKind};
use std::collections::HashMap;

pub fn parse_program(tokens: &[Token]) -> Result<Program, Vec<Diagnostic>> {
    let mut p = Parser {
        program: Program::new(),
        diags: Vec::new(),
        current_fn: None,
        current_block: None,
        labels: Vec::new(),
        in_macro: false,
        pending_loc: None,
        def_values: HashMap::new(),
    };
    let mut start = 0usize;
    for i in 0..=tokens.len() {
        let at_end = i == tokens.len();
        if !at_end && tokens[i].kind != TokenKind::Newline {
            continue;
        }
        let line: Vec<Token> = tokens[start..i]
            .iter()
            .filter(|t| t.kind != TokenKind::Comment && t.kind != TokenKind::Eof)
            .cloned()
            .collect();
        start = i + 1;
        if line.is_empty() {
            continue;
        }
        p.parse_line(&line);
    }
    if p.diags.is_empty() {
        Ok(p.program)
    } else {
        Err(p.diags)
    }
}

pub fn parse_instruction_line(text: &str, function: &str) -> Option<sa_ir::Instruction> {
    let tokens = tokenize(text, "inline.sa").ok()?;
    let line: Vec<Token> = tokens
        .into_iter()
        .filter(|t| !matches!(t.kind, TokenKind::Newline | TokenKind::Eof))
        .collect();
    if line.is_empty() {
        return None;
    }
    let mut diags = Vec::new();
    parse_instr::parse_instruction(&line, function, &mut diags)
}

pub fn parse_source(src: &str, file: &str) -> Result<Program, Vec<Diagnostic>> {
    parse_source_with_target(src, file, "native")
}

pub fn parse_source_with_target(
    src: &str,
    file: &str,
    target: &str,
) -> Result<Program, Vec<Diagnostic>> {
    match tokenize(src, file) {
        Ok(tokens) => parse_program(&select_target_lines(&tokens, target)),
        Err(d) => Err(vec![d]),
    }
}

fn select_target_lines(tokens: &[Token], target: &str) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut in_block = false;
    let mut active = true;
    let mut start = 0usize;
    for i in 0..tokens.len() {
        if tokens[i].kind != TokenKind::Newline {
            continue;
        }
        let line = &tokens[start..=i];
        start = i + 1;
        let marker = line
            .iter()
            .find(|t| t.kind == TokenKind::Comment && t.text.contains("@target"));
        if let Some(marker) = marker {
            let text = marker.text.as_str();
            if text.contains("@target-begin") {
                in_block = true;
                active = false;
                continue;
            }
            if text.contains("@target-end") {
                in_block = false;
                active = true;
                continue;
            }
            if in_block {
                if let Some((_, name)) = text.split_once("@target") {
                    active = name.trim() == target;
                }
                continue;
            }
        }
        if !in_block || active {
            out.extend_from_slice(line);
        }
    }
    if start < tokens.len() {
        if !in_block || active {
            out.extend_from_slice(&tokens[start..]);
        }
    }
    out
}

struct Parser {
    program: Program,
    diags: Vec<Diagnostic>,
    current_fn: Option<usize>,
    current_block: Option<usize>,
    labels: Vec<String>,
    in_macro: bool,
    /// sci flattener `pending_loc`: a `#loc` directive retargets the next
    /// emitted instruction to its upstream source position (one-shot).
    pending_loc: Option<SourceLoc>,
    /// B6: first `#def` value per name in this file. sci's DefDict reports
    /// DuplicateDef when a later definition evaluates to a different value;
    /// identical redefinitions (same value reached twice) are tolerated.
    def_values: HashMap<String, String>,
}

impl Parser {
    fn parse_line(&mut self, line: &[Token]) {
        let head: String = line.iter().map(|t| t.text.as_str()).collect();
        if head.starts_with("[MACRO]") {
            self.leave_function();
            self.in_macro = true;
            return;
        }
        if self.in_macro {
            if head.starts_with("[END_MACRO]") {
                self.in_macro = false;
            }
            return;
        }
        if self.current_fn.is_none() && line[0].is_ident("EXPAND") {
            return;
        }
        match line[0].kind {
            TokenKind::Directive => self.parse_directive(line),
            TokenKind::HashDirective => {
                // `#loc` is an inline upstream-location marker (sci flattener
                // emits it inside function bodies); it must not terminate the
                // current function. Other directives remain top-level.
                if line[0].text.as_str() != "#loc" {
                    self.leave_function();
                }
                match line[0].text.as_str() {
                    "#def" => {
                        if let Some(d) = parse_header::parse_def(line) {
                            match self.def_values.get(&d.name) {
                                Some(previous) if previous != &d.value => {
                                    // B6: sci putExpression evaluates every .def
                                    // and reports DuplicateDef on a value clash.
                                    let diag = Diagnostic::new(
                                        Trap::DuplicateDef,
                                        "duplicate definition detected during flattening",
                                        d.loc.clone(),
                                    )
                                    .with_hint(format!("conflicting definition '{}'", d.name));
                                    self.diags.push(diag);
                                }
                                Some(_) => {}
                                None => {
                                    self.def_values.insert(d.name.clone(), d.value.clone());
                                }
                            }
                            self.program.defs.push(d);
                        } else {
                            self.diags.push(err(line, "invalid #def declaration"));
                        }
                    }
                    "#loc" => {
                        if let Some(l) = parse_header::parse_loc(line) {
                            // C5: the directive retargets the next emitted
                            // instruction to its upstream source position.
                            self.pending_loc = Some(SourceLoc::new(
                                l.file.clone(),
                                l.line,
                                l.column,
                            ));
                            self.program.locs.push(l);
                        } else {
                            self.diags.push(err(line, "invalid #loc declaration"));
                        }
                    }
                    "#npm_bind" => {
                        if let Some(b) = parse_header::parse_npm_bind(line) {
                            self.program.npm_binds.push(b);
                        } else {
                            self.diags.push(err(line, "invalid #npm_bind declaration"));
                        }
                    }
                    "#version" => {
                        // sci line_classifier.zig:445-455: `#version <digits>`
                        // is a metadata line (no emitted instruction). Other
                        // forms are unknown/invalid.
                        let valid = line.len() == 2
                            && line[1].kind == TokenKind::Number
                            && !line[1].text.is_empty()
                            && line[1].text.bytes().all(|b| b.is_ascii_digit());
                        if !valid {
                            self.diags.push(err(line, "invalid #version declaration"));
                        }
                    }
                    other => self.diags.push(err(line, &format!("unknown hash directive {other}"))),
                }
            }
            TokenKind::Label => self.start_block(line),
            _ => self.parse_body_line(line),
        }
    }

    fn parse_directive(&mut self, line: &[Token]) {
        self.leave_function();
        match line[0].text.as_str() {
            "@import" => {
                if let Some(i) = parse_header::parse_import(line) {
                    self.program.imports.push(i);
                } else {
                    self.diags.push(err(line, "invalid @import declaration"));
                }
            }
            "@const" => match parse_header::parse_const(line) {
                Ok(c) => self.program.consts.push(c),
                // B14 (sci parity): malformed @const literals are
                // ForbiddenSyntax(1001) "InvalidLiteral", not the generic
                // ParseError(2001) used for other directive syntax errors.
                Err(_) => {
                    let loc = line.first().map(|t| t.loc.clone()).unwrap_or_default();
                    self.diags.push(Diagnostic::new(
                        Trap::ForbiddenSyntax,
                        "InvalidLiteral",
                        loc,
                    ));
                }
            },
            "@extern" => match parse_header::parse_signature(line, FunctionKind::External) {
                Ok(sig) => self.program.externs.push(sig),
                Err(m) => self.diags.push(err(line, &m)),
            },
            other => {
                let kind = match other {
                    "@export" => FunctionKind::Exported,
                    "@test" => FunctionKind::TestFunc,
                    "@ffi_wrapper" => FunctionKind::FfiWrapper,
                    "@gpu" => FunctionKind::Gpu,
                    _ => FunctionKind::Normal,
                };
                match parse_header::parse_signature(line, kind) {
                    Ok(sig) => {
                        let loc = line[0].loc.clone();
                        self.program.functions.push(Function::new(sig, loc));
                        self.current_fn = Some(self.program.functions.len() - 1);
                        self.current_block = None;
                        self.labels.clear();
                    }
                    Err(m) => self.diags.push(err(line, &m)),
                }
            }
        }
    }

    fn leave_function(&mut self) {
        self.current_fn = None;
        self.current_block = None;
        self.labels.clear();
    }

    fn start_block(&mut self, line: &[Token]) {
        let Some(fi) = self.current_fn else {
            self.diags.push(err(line, "label outside of function"));
            return;
        };
        let label = line[0].text.clone();
        if self.labels.contains(&label) {
            let mut d = err(line, &format!("duplicate label {label}"));
            d.trap = Trap::DuplicateLabel;
            d = d.in_function(self.program.functions[fi].sig.display_name());
            self.diags.push(d);
            return;
        }
        self.labels.push(label.clone());
        let block = Block::new(label, line[0].loc.clone());
        self.program.functions[fi].blocks.push(block);
        self.current_block = Some(self.program.functions[fi].blocks.len() - 1);
    }

    fn parse_body_line(&mut self, line: &[Token]) {
        let Some(fi) = self.current_fn else {
            self.diags.push(err(line, "instruction outside of function"));
            return;
        };
        let fname = self.program.functions[fi].sig.display_name().to_string();
        let Some(mut ins) = parse_instr::parse_instruction(line, &fname, &mut self.diags) else {
            return;
        };
        // One-shot upstream retargeting, consuming the pending #loc.
        if let Some(upstream) = self.pending_loc.take() {
            ins.loc = upstream;
        }
        if self.current_block.is_none() {
            let block = Block::new("L_ENTRY", line[0].loc.clone());
            self.program.functions[fi].blocks.push(block);
            self.current_block = Some(self.program.functions[fi].blocks.len() - 1);
        }
        let bi = self.current_block.unwrap();
        self.program.functions[fi].blocks[bi].instrs.push(ins);
    }
}

fn err(line: &[Token], message: &str) -> Diagnostic {
    let loc = line.first().map(|t| t.loc.clone()).unwrap_or_default();
    Diagnostic::new(Trap::ParseError, message, loc)
}

pub(crate) fn error_at(loc: SourceLoc, trap: Trap, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(trap, message, loc)
}

pub(crate) fn join(tokens: &[Token]) -> String {
    let mut out = String::new();
    let mut prev: Option<&Token> = None;
    for t in tokens {
        if let Some(p) = prev {
            let tight_before = matches!(
                t.kind,
                TokenKind::Comma
                    | TokenKind::Colon
                    | TokenKind::RParen
                    | TokenKind::RBrace
                    | TokenKind::Other('.')
                    | TokenKind::Other(']')
            );
            let tight_after = matches!(
                p.kind,
                TokenKind::LParen
                    | TokenKind::Amp
                    | TokenKind::Caret
                    | TokenKind::Star
                    | TokenKind::Bang
                    | TokenKind::Question
                    | TokenKind::Colon
                    | TokenKind::Plus
                    | TokenKind::Minus
            );
            if !tight_before && !tight_after {
                out.push(' ');
            }
        }
        out.push_str(&t.text);
        prev = Some(t);
    }
    out
}

pub(crate) fn split_commas(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut parts: Vec<Vec<Token>> = Vec::new();
    let mut cur: Vec<Token> = Vec::new();
    let mut depth = 0i32;
    for t in tokens {
        match t.kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::Other('[') => depth += 1,
            TokenKind::RParen | TokenKind::RBrace | TokenKind::Other(']') => depth -= 1,
            TokenKind::Comma if depth == 0 => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(t.clone());
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

pub(crate) fn top_equal(tokens: &[Token]) -> Option<usize> {
    let mut depth = 0i32;
    for (i, t) in tokens.iter().enumerate() {
        match t.kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::Other('[') => depth += 1,
            TokenKind::RParen | TokenKind::RBrace | TokenKind::Other(']') => depth -= 1,
            TokenKind::Equal if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

pub(crate) fn take_type_suffix(tokens: &mut Vec<Token>) -> Option<sa_ir::PrimType> {
    let mut depth = 0i32;
    for i in 0..tokens.len() {
        match tokens[i].kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::Other('[') => depth += 1,
            TokenKind::RParen | TokenKind::RBrace | TokenKind::Other(']') => depth -= 1,
            _ => {}
        }
        if depth == 0
            && tokens[i].is_ident("as")
            && i + 1 < tokens.len()
        {
            if let Some(ty) = sa_ir::PrimType::parse(&tokens[i + 1].text) {
                tokens.drain(i..i + 2);
                return Some(ty);
            }
        }
    }
    None
}

pub(crate) fn take_orderings(tokens: &mut Vec<Token>) -> Vec<sa_ir::AtomicOrdering> {
    let mut found = Vec::new();
    while let Some(last) = tokens.last() {
        if let Some(o) = sa_ir::AtomicOrdering::parse(&last.text) {
            found.push(o);
            tokens.pop();
        } else {
            break;
        }
    }
    found.reverse();
    found
}

pub(crate) fn string_value(text: &str) -> String {
    match sa_ir::Operand::parse(text) {
        sa_ir::Operand::Utf8(s) => s,
        sa_ir::Operand::Bytes(b) => String::from_utf8_lossy(&b).into_owned(),
        _ => text
            .trim()
            .trim_start_matches("utf8:")
            .trim_matches('"')
            .to_string(),
    }
}

pub(crate) fn parse_call_args(tokens: &[Token]) -> Vec<sa_ir::CallArg> {
    split_commas(tokens)
        .iter()
        .map(|part| {
            if part.is_empty() {
                return sa_ir::CallArg::new(sa_ir::CapPrefix::ByValue, sa_ir::Operand::Undef);
            }
            let (cap, rest) = match part[0].kind {
                TokenKind::Amp => (sa_ir::CapPrefix::Borrow, &part[1..]),
                TokenKind::Caret => (sa_ir::CapPrefix::Move, &part[1..]),
                TokenKind::Star => (sa_ir::CapPrefix::Raw, &part[1..]),
                _ => (sa_ir::CapPrefix::ByValue, &part[..]),
            };
            let rest = if rest.first().map(|t| t.is_ident("mut")).unwrap_or(false) {
                &rest[1..]
            } else {
                rest
            };
            let value = if rest.is_empty() {
                sa_ir::Operand::Undef
            } else {
                sa_ir::Operand::parse(&join(rest))
            };
            sa_ir::CallArg::new(cap, value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{InstructionKind, PrimType};

    fn first_program(src: &str) -> Program {
        parse_source(src, "test.sa").unwrap()
    }

    #[test]
    fn parses_full_program() {
        let src = "@import \"io.sa\"\n@const X = utf8:\"hi\\n\"\n#def OFF = +4\n@extern f(x: i32) -> i32\n@main() -> i32:\nL_ENTRY:\n  a = alloc 8\n  store a+OFF, 5 as i32\n  b = load a+0 as i32\n  call @f(b)\n  return 0\n";
        let p = first_program(src);
        assert_eq!(p.imports.len(), 1);
        assert_eq!(p.imports[0].path, "io.sa");
        assert_eq!(p.consts.len(), 1);
        assert_eq!(p.consts[0].value, sa_ir::ConstValue::Utf8("hi\n".into()));
        assert_eq!(p.defs[0].value, "+4");
        assert_eq!(p.externs.len(), 1);
        assert_eq!(p.functions.len(), 1);
        let f = p.function("main").unwrap();
        assert_eq!(f.blocks.len(), 1);
        assert_eq!(f.blocks[0].label, "L_ENTRY");
        assert_eq!(f.blocks[0].instrs.len(), 5);
        assert_eq!(f.blocks[0].instrs[0].kind, InstructionKind::Alloc { size: sa_ir::Operand::Int(8) });
        assert_eq!(f.blocks[0].instrs[2].ty, Some(PrimType::I32));
    }

    #[test]
    fn implicit_entry_block_and_branches() {
        let p = first_program("@f():\n  x = 1 as i32\n  br x -> L_A, L_B\nL_A:\n  jmp L_B\nL_B:\n  return\n");
        let f = p.function("f").unwrap();
        assert_eq!(f.labels(), vec!["L_ENTRY", "L_A", "L_B"]);
        assert_eq!(f.blocks[0].instrs.len(), 2);
        assert_eq!(f.blocks[1].instrs.len(), 1);
    }

    #[test]
    fn duplicate_labels_error() {
        let errs = parse_source("@f():\nL_A:\n  nop\nL_A:\n  nop\n", "t.sa").unwrap_err();
        assert!(errs.iter().any(|d| d.trap == Trap::DuplicateLabel));
    }

    #[test]
    fn unsupported_instruction_is_reported() {
        let errs = parse_source("@f():\nL_ENTRY:\n  frobnicate x, y\n  nop\n", "t.sa").unwrap_err();
        assert!(errs.iter().any(|d| d.trap == Trap::UnsupportedInstruction));
    }

    #[test]
    fn tokenize_error_becomes_single_diagnostic() {
        let errs = parse_source("@f():\n  x = \"oops\n", "t.sa").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].trap, Trap::ParseError);
    }

    #[test]
    fn top_level_expand_is_ignored() {
        let p = first_program("EXPAND DEFINE_THING x\n@f():\nL_ENTRY:\n  nop\n");
        assert_eq!(p.functions.len(), 1);
        assert_eq!(p.function("f").unwrap().all_instructions().len(), 1);
    }

    #[test]
    fn comments_and_blank_lines_skipped() {
        let p = first_program("// lead\n\n@f():\n// body\nL_ENTRY:\n  nop\n");
        assert_eq!(p.function("f").unwrap().all_instructions().len(), 1);
    }

    #[test]
    fn target_variants_select_native_by_default() {
        let src = "@f() -> i32:\nL_ENTRY:\n// @target-begin\n// @target native\n  a = add 1, 2\n// @target wasm32\n  a = add 3, 4\n// @target-end\n  return a\n";
        let native = first_program(src);
        let text = native.function("f").unwrap().to_text();
        assert!(text.contains("add 1, 2"));
        assert!(!text.contains("add 3, 4"));
        let wasm = parse_source_with_target(src, "t.sa", "wasm32").unwrap();
        let text = wasm.function("f").unwrap().to_text();
        assert!(text.contains("add 3, 4"));
        assert!(!text.contains("add 1, 2"));
    }

    #[test]
    fn duplicate_def_with_different_value_is_reported() {
        let errs = parse_source(
            "#def A = 1\n#def A = 2\n@f():\nL_ENTRY:\n  nop\n",
            "t.sa",
        )
        .unwrap_err();
        let d = errs
            .iter()
            .find(|d| d.trap == Trap::DuplicateDef)
            .expect("DuplicateDef diagnostic");
        assert_eq!(d.message, "duplicate definition detected during flattening");
        assert_eq!(d.loc.line, 2);
        assert_eq!(d.hint.as_deref(), Some("conflicting definition 'A'"));
    }

    #[test]
    fn duplicate_def_with_same_value_is_tolerated() {
        let p = parse_source(
            "#def A = 1\n#def A = 1\n@f():\nL_ENTRY:\n  nop\n",
            "t.sa",
        )
        .expect("identical redefinition is tolerated");
        assert_eq!(p.defs.len(), 2);
    }

    #[test]
    fn utility_join_and_split() {
        let toks = tokenize("utf8:\"a\\n\", x+4", "t.sa").unwrap();
        let line: Vec<Token> = toks.iter().filter(|t| t.kind != TokenKind::Eof).cloned().collect();
        assert_eq!(join(&line), "utf8:\"a\\n\", x +4");
        assert_eq!(split_commas(&line).len(), 2);
    }
}
