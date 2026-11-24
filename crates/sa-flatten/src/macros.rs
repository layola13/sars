use crate::walk::rewrite_operands;
use sa_ir::{DefDecl, Diagnostic, Operand, Program, SourceLoc, Trap};
use std::collections::HashSet;

pub(crate) fn expand_program(
    program: &mut Program,
    diags: &mut Vec<Diagnostic>,
    max_depth: usize,
) {
    let known = collect_known(program);
    let defs = program.defs.clone();
    for function in &mut program.functions {
        let func = function.sig.name.clone();
        for block in &mut function.blocks {
            for instruction in &mut block.instrs {
                let loc = instruction.loc.clone();
                if let Some(dest) = instruction.dest.take() {
                    let next = expand_text(
                        &dest, &defs, &known, max_depth, diags, &loc, Some(&func), &[], 0,
                    );
                    instruction.dest = Some(next);
                }
                let raw = expand_text(
                    &instruction.raw,
                    &defs,
                    &known,
                    max_depth,
                    diags,
                    &loc,
                    Some(&func),
                    &[],
                    0,
                );
                instruction.raw = raw;
                rewrite_operands(&mut instruction.kind, &mut |operand| {
                    expand_operand(
                        operand, &defs, &known, max_depth, diags, &loc, Some(&func),
                    )
                });
            }
        }
    }
}

fn collect_known(program: &Program) -> HashSet<String> {
    let mut known = HashSet::new();
    for function in &program.functions {
        known.insert(function.sig.name.clone());
    }
    for constant in &program.consts {
        known.insert(constant.name.clone());
    }
    for extern_sig in &program.externs {
        known.insert(extern_sig.name.clone());
    }
    for import in &program.imports {
        known.insert(import.path.clone());
        if let Some(alias) = &import.alias {
            known.insert(alias.clone());
        }
    }
    known
}

fn find_def<'a>(defs: &'a [DefDecl], name: &str) -> Option<&'a DefDecl> {
    defs.iter().find(|def| def.name == name)
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '@' || c == '.'
}

fn looks_like_macro(name: &str) -> bool {
    let mut has_letter = false;
    for c in name.chars() {
        if c.is_alphabetic() {
            if !c.is_uppercase() {
                return false;
            }
            has_letter = true;
        }
    }
    has_letter
}

fn emit(
    diags: &mut Vec<Diagnostic>,
    trap: Trap,
    message: String,
    loc: &SourceLoc,
    func: Option<&str>,
) {
    let mut diagnostic = Diagnostic::new(trap, message, loc.clone());
    if let Some(func) = func {
        diagnostic = diagnostic.in_function(func);
    }
    if !diags.contains(&diagnostic) {
        diags.push(diagnostic);
    }
}

fn expand_text(
    text: &str,
    defs: &[DefDecl],
    known: &HashSet<String>,
    max_depth: usize,
    diags: &mut Vec<Diagnostic>,
    loc: &SourceLoc,
    func: Option<&str>,
    stack: &[String],
    depth: usize,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut iter = text.char_indices().peekable();
    while let Some((start, c)) = iter.next() {
        if !is_token_char(c) {
            out.push(c);
            continue;
        }
        let mut end = start + c.len_utf8();
        while let Some(&(next, following)) = iter.peek() {
            if !is_token_char(following) {
                break;
            }
            iter.next();
            end = next + following.len_utf8();
        }
        let token = &text[start..end];
        let (has_at, name) = match token.strip_prefix('@') {
            Some(rest) => (true, rest),
            None => (false, token),
        };
        if let Some(def) = find_def(defs, name) {
            if stack.iter().any(|item| item == name) || depth >= max_depth {
                emit(
                    diags,
                    Trap::MacroRecursionLimit,
                    format!("macro '{}' exceeded expansion depth limit", name),
                    loc,
                    func,
                );
                out.push_str(token);
            } else {
                let mut next_stack = Vec::with_capacity(stack.len() + 1);
                next_stack.extend_from_slice(stack);
                next_stack.push(name.to_string());
                let value = expand_text(
                    &def.value,
                    defs,
                    known,
                    max_depth,
                    diags,
                    &def.loc,
                    func,
                    &next_stack,
                    depth + 1,
                );
                out.push_str(&value);
            }
        } else {
            if has_at && !known.contains(name) && looks_like_macro(name) {
                emit(
                    diags,
                    Trap::UndefinedMacro,
                    format!("undefined macro '@{}'", name),
                    loc,
                    func,
                );
            }
            out.push_str(token);
        }
    }
    out
}

fn expand_operand(
    operand: &Operand,
    defs: &[DefDecl],
    known: &HashSet<String>,
    max_depth: usize,
    diags: &mut Vec<Diagnostic>,
    loc: &SourceLoc,
    func: Option<&str>,
) -> Operand {
    match operand {
        Operand::Reg(text) | Operand::Label(text) => {
            let expanded = expand_text(text, defs, known, max_depth, diags, loc, func, &[], 0);
            Operand::parse(&expanded)
        }
        Operand::Symbol(name) => {
            if let Some(def) = find_def(defs, name) {
                let stack = vec![name.clone()];
                let value = expand_text(
                    &def.value,
                    defs,
                    known,
                    max_depth,
                    diags,
                    &def.loc,
                    func,
                    &stack,
                    1,
                );
                Operand::parse(&value)
            } else {
                if !known.contains(name) && looks_like_macro(name) {
                    emit(
                        diags,
                        Trap::UndefinedMacro,
                        format!("undefined macro '@{}'", name),
                        loc,
                        func,
                    );
                }
                Operand::Symbol(name.clone())
            }
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{Block, Function, Instruction, InstructionKind, OpKind, Signature};

    fn def(name: &str, value: &str) -> DefDecl {
        DefDecl {
            name: name.to_string(),
            value: value.to_string(),
            loc: SourceLoc::new("defs.sa", 1, 1),
        }
    }

    fn program(defs: Vec<DefDecl>, kind: InstructionKind, raw: &str) -> Program {
        let loc = SourceLoc::new("main.sa", 2, 3);
        let mut block = Block::new("entry", loc.clone());
        block
            .instrs
            .push(Instruction::new(kind, loc.clone(), raw));
        let mut function = Function::new(Signature::new("main"), loc);
        function.blocks.push(block);
        let mut program = Program::new();
        program.defs = defs;
        program.functions.push(function);
        program
    }

    fn op(args: Vec<Operand>) -> InstructionKind {
        InstructionKind::Op {
            op: OpKind::Add,
            args,
        }
    }

    #[test]
    fn expands_bare_and_at_macro_references() {
        let mut program = program(
            vec![def("SRC", "r7"), def("SIZE", "64")],
            op(vec![Operand::reg("SRC"), Operand::Symbol("SIZE".into())]),
            "x = add SRC, @SIZE",
        );
        let mut diags = Vec::new();
        expand_program(&mut program, &mut diags, 32);
        match &program.functions[0].blocks[0].instrs[0].kind {
            InstructionKind::Op { args, .. } => {
                assert_eq!(args[0], Operand::reg("r7"));
                assert_eq!(args[1], Operand::Int(64));
            }
            _ => panic!("wrong kind"),
        }
        assert_eq!(
            program.functions[0].blocks[0].instrs[0].raw,
            "x = add r7, 64"
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn expands_dest_field() {
        let mut program = program(
            vec![def("DST", "r9")],
            InstructionKind::Assign {
                src: Operand::reg("r0"),
            },
            "DST = r0",
        );
        program.functions[0].blocks[0].instrs[0].dest = Some("DST".to_string());
        let mut diags = Vec::new();
        expand_program(&mut program, &mut diags, 32);
        assert_eq!(
            program.functions[0].blocks[0].instrs[0].dest.as_deref(),
            Some("r9")
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn detects_macro_cycles() {
        let mut program = program(
            vec![def("A", "@B"), def("B", "A")],
            op(vec![Operand::reg("A")]),
            "x = add A, r0",
        );
        let mut diags = Vec::new();
        expand_program(&mut program, &mut diags, 32);
        assert!(diags
            .iter()
            .any(|d| d.trap == Trap::MacroRecursionLimit));
    }

    #[test]
    fn honors_depth_limit() {
        let mut program = program(
            vec![def("A", "B"), def("B", "C"), def("C", "D"), def("D", "1")],
            op(vec![Operand::reg("A")]),
            "x = add A, r0",
        );
        let mut diags = Vec::new();
        expand_program(&mut program, &mut diags, 2);
        assert!(diags
            .iter()
            .any(|d| d.trap == Trap::MacroRecursionLimit));
    }

    #[test]
    fn reports_undefined_macro_conservatively() {
        let mut program = program(
            vec![def("KNOWN", "1")],
            op(vec![Operand::Symbol("MISSING".into()), Operand::Symbol("lower".into())]),
            "x = add @MISSING, @lower",
        );
        let mut diags = Vec::new();
        expand_program(&mut program, &mut diags, 32);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].trap, Trap::UndefinedMacro);
        assert_eq!(diags[0].loc.file, "main.sa");
    }

    #[test]
    fn known_symbols_are_not_reported() {
        let mut program = program(
            vec![],
            op(vec![Operand::Symbol("main".into())]),
            "call @main()",
        );
        let mut diags = Vec::new();
        expand_program(&mut program, &mut diags, 32);
        assert!(diags.is_empty());
    }
}
