use crate::walk::rewrite_operands;
use sa_ir::{DefDecl, Diagnostic, Operand, Program, SourceLoc, Trap};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub(crate) struct MacroDef {
    pub(crate) name: String,
    pub(crate) params: Vec<String>,
    pub(crate) body: Vec<String>,
}

fn is_param_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub(crate) fn collect_macros(text: &str, macros: &mut HashMap<String, MacroDef>) {
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("[MACRO]") else {
            continue;
        };
        let rest = rest.trim();
        let mut end = 0usize;
        for (i, c) in rest.char_indices() {
            if is_param_char(c) {
                end = i + c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            continue;
        }
        let name = rest[..end].to_string();
        let tail = rest[end..].trim();
        let params = if tail.is_empty() {
            Vec::new()
        } else {
            tail.split(',')
                .map(|param| param.trim().trim_start_matches('%').to_string())
                .filter(|param| !param.is_empty())
                .collect()
        };
        let mut body = Vec::new();
        for body_line in lines.by_ref() {
            if body_line.trim() == "[END_MACRO]" {
                break;
            }
            let line = body_line.trim();
            if !line.is_empty() {
                body.push(line.to_string());
            }
        }
        macros.insert(name.clone(), MacroDef { name, params, body });
    }
}

pub(crate) fn expand_text(
    text: &str,
    macros: &HashMap<String, MacroDef>,
    depth: usize,
    max_depth: usize,
    diags: &mut Vec<Diagnostic>,
) -> String {
    let mut stack = Vec::new();
    let mut counter = 0usize;
    expand_lines(
        text, macros, depth, max_depth, diags, &mut stack, &mut counter,
    )
}

fn emit_macro(diags: &mut Vec<Diagnostic>, trap: Trap, message: String) {
    let diagnostic = Diagnostic::new(trap, message, SourceLoc::unknown());
    if !diags.contains(&diagnostic) {
        diags.push(diagnostic);
    }
}

fn expand_lines(
    text: &str,
    macros: &HashMap<String, MacroDef>,
    depth: usize,
    max_depth: usize,
    diags: &mut Vec<Diagnostic>,
    stack: &mut Vec<String>,
    counter: &mut usize,
) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("EXPAND") {
            if rest.starts_with(char::is_whitespace) {
                out.push_str(&expand_call(
                    rest.trim(), macros, depth, max_depth, diags, stack, counter,
                ));
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn expand_call(
    rest: &str,
    macros: &HashMap<String, MacroDef>,
    depth: usize,
    max_depth: usize,
    diags: &mut Vec<Diagnostic>,
    stack: &mut Vec<String>,
    counter: &mut usize,
) -> String {
    let mut parts = rest.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("").trim();
    let args_text = parts.next().unwrap_or("").trim();
    let def = match macros.get(name) {
        Some(def) => def,
        None => {
            emit_macro(
                diags,
                Trap::UndefinedMacro,
                format!("undefined macro '{}'", name),
            );
            return String::new();
        }
    };
    let args = split_args(args_text);
    if args.len() != def.params.len() {
        emit_macro(
            diags,
            Trap::ParseError,
            format!(
                "macro '{}' expects {} arguments, got {}",
                def.name,
                def.params.len(),
                args.len()
            ),
        );
        return String::new();
    }
    if stack.iter().any(|item| item == name) || depth >= max_depth {
        emit_macro(
            diags,
            Trap::MacroRecursionLimit,
            format!("macro '{}' exceeded expansion depth limit", name),
        );
        return String::new();
    }
    *counter += 1;
    let suffix = format!("__m{}", *counter);
    stack.push(name.to_string());
    let mut lines: Vec<String> = Vec::new();
    for body_line in &def.body {
        let substituted = substitute(body_line, &def.params, &args);
        let expanded = expand_lines(
            &substituted, macros, depth + 1, max_depth, diags, stack, counter,
        );
        for line in expanded.lines() {
            lines.push(line.to_string());
        }
    }
    stack.pop();
    let labels = label_defs(&lines);
    if !labels.is_empty() {
        apply_label_hygiene(&mut lines, &labels, &suffix);
    }
    let mut out = String::with_capacity(lines.len() * 32);
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn split_args(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in text.chars() {
        if let Some(q) = quote {
            cur.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                cur.push(c);
            }
            '(' | '[' | '{' => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn substitute(line: &str, params: &[String], args: &[String]) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let mut best: Option<(usize, usize)> = None;
            for (pos, param) in params.iter().enumerate() {
                if param.is_empty() || !line[i + 1..].starts_with(param) {
                    continue;
                }
                if best.map_or(true, |(_, len)| param.len() > len) {
                    best = Some((pos, param.len()));
                }
            }
            if let Some((pos, len)) = best {
                out.push_str(&args[pos]);
                i += 1 + len;
                continue;
            }
        }
        let c = line[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn label_defs(lines: &[String]) -> HashSet<String> {
    let mut out = HashSet::new();
    for line in lines {
        let trimmed = line.trim();
        let Some(name) = trimmed.strip_suffix(':') else {
            continue;
        };
        if name.starts_with("L_") && name.chars().all(is_param_char) {
            out.insert(name.to_string());
        }
    }
    out
}

fn is_branch(line: &str) -> bool {
    line == "jmp"
        || line.starts_with("jmp ")
        || line.starts_with("br ")
        || line.starts_with("br_null ")
}

fn apply_label_hygiene(lines: &mut [String], labels: &HashSet<String>, suffix: &str) {
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_suffix(':') {
            if labels.contains(name) {
                let indent = &line[..line.len() - line.trim_start().len()];
                *line = format!("{}{}{}:", indent, name, suffix);
                continue;
            }
        }
        if is_branch(trimmed) {
            *line = rewrite_targets(line, labels, suffix);
        }
    }
}

fn rewrite_targets(line: &str, labels: &HashSet<String>, suffix: &str) -> String {
    let mut out = String::with_capacity(line.len() + 16);
    let mut token = String::new();
    for c in line.chars() {
        if is_param_char(c) {
            token.push(c);
        } else {
            push_target(&mut out, &token, labels, suffix);
            token.clear();
            out.push(c);
        }
    }
    push_target(&mut out, &token, labels, suffix);
    out
}

fn push_target(out: &mut String, token: &str, labels: &HashSet<String>, suffix: &str) {
    out.push_str(token);
    if labels.contains(token) {
        out.push_str(suffix);
    }
}

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
                    let next = expand_defs_text(
                        &dest, &defs, &known, max_depth, diags, &loc, Some(&func), &[], 0,
                    );
                    instruction.dest = Some(next);
                }
                let raw = expand_defs_text(
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

fn expand_defs_text(
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
            format!("macro '{}' exceeded expansion depth limit", def.name),
                    loc,
                    func,
                );
                out.push_str(token);
            } else {
                let mut next_stack = Vec::with_capacity(stack.len() + 1);
                next_stack.extend_from_slice(stack);
                next_stack.push(name.to_string());
                let value = expand_defs_text(
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
            let expanded = expand_defs_text(text, defs, known, max_depth, diags, loc, func, &[], 0);
            Operand::parse(&expanded)
        }
        Operand::Symbol(name) => {
            if let Some(def) = find_def(defs, name) {
                let stack = vec![name.clone()];
                let value = expand_defs_text(
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

    fn macros(src: &str) -> HashMap<String, MacroDef> {
        let mut out = HashMap::new();
        collect_macros(src, &mut out);
        out
    }

    #[test]
    fn collects_macro_definitions() {
        let defs = macros("[MACRO] SET %out, %value\n    %out = add %value, 1\n[END_MACRO]\n");
        let def = &defs["SET"];
        assert_eq!(def.params, vec!["out", "value"]);
        assert_eq!(def.body, vec!["%out = add %value, 1"]);
    }

    #[test]
    fn expands_parameters_as_tokens_and_prefixes() {
        let defs = macros(
            "[MACRO] WRAP %out_opt, %value\n    %out_opt = add %value, 1\n    L_DONE_%out_opt:\n    br done -> L_DONE_%out_opt, L_FAIL_%out_opt\nL_FAIL_%out_opt:\n[END_MACRO]\n",
        );
        let mut diags = Vec::new();
        let expanded = expand_text("EXPAND WRAP left, 2\n", &defs, 0, 32, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        assert!(expanded.contains("left = add 2, 1"));
        assert!(expanded.contains("L_DONE_left__m1:"));
        assert!(expanded.contains("L_FAIL_left__m1:"));
        assert!(expanded.contains("br done -> L_DONE_left__m1, L_FAIL_left__m1"));
    }

    #[test]
    fn nested_expansion_substitutes_all_parameters() {
        let defs = macros(
            "[MACRO] INNER %out\n    %out = load %out+0 as i32\n[END_MACRO]\n[MACRO] OUTER %dst\n    EXPAND INNER %dst\n[END_MACRO]\n",
        );
        let mut diags = Vec::new();
        let expanded = expand_text("EXPAND OUTER r9\n", &defs, 0, 32, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        assert!(expanded.contains("r9 = load r9+0 as i32"));
        assert!(!expanded.contains("EXPAND"));
    }

    #[test]
    fn reports_unknown_macro_and_argument_mismatch() {
        let defs = macros("[MACRO] ONE %a\n    nop\n[END_MACRO]\n");
        let mut diags = Vec::new();
        let _ = expand_text(
            "EXPAND MISSING 1\nEXPAND ONE 1, 2\n",
            &defs,
            0,
            32,
            &mut diags,
        );
        assert_eq!(diags.len(), 2, "{diags:?}");
        assert_eq!(diags[0].trap, Trap::UndefinedMacro);
        assert_eq!(diags[1].trap, Trap::ParseError);
    }

    #[test]
    fn detects_macro_call_cycles() {
        let defs = macros(
            "[MACRO] A\n    EXPAND B\n[END_MACRO]\n[MACRO] B\n    EXPAND A\n[END_MACRO]\n",
        );
        let mut diags = Vec::new();
        let _ = expand_text("EXPAND A\n", &defs, 0, 4, &mut diags);
        assert!(diags
            .iter()
            .any(|diag| diag.trap == Trap::MacroRecursionLimit));
    }

    #[test]
    fn substitutes_parameter_prefixes_inside_identifiers() {
        let defs = macros("[MACRO] CTX %name\n    #def %name_SIZE = 32\n[END_MACRO]\n");
        let mut diags = Vec::new();
        let expanded = expand_text("EXPAND CTX Run\n", &defs, 0, 32, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        assert!(expanded.contains("#def Run_SIZE = 32"));
    }

    #[test]
    fn substitutes_multiple_parameters_in_one_label() {
        let defs = macros(
            "[MACRO] P %a, %b\nL_X_%a_%b:\n    jmp L_X_%a_%b\n[END_MACRO]\n",
        );
        let mut diags = Vec::new();
        let expanded = expand_text("EXPAND P 1, Z\n", &defs, 0, 32, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        assert!(expanded.contains("L_X_1_Z__m1:"));
        assert!(expanded.contains("jmp L_X_1_Z__m1"));
    }

    #[test]
    fn label_suffixes_are_unique_per_expansion() {
        let defs = macros(
            "[MACRO] BRANCH %out\n    br %out -> L_OK_%out, L_BAD_%out\nL_OK_%out:\n    jmp L_BAD_%out\nL_BAD_%out:\n[END_MACRO]\n",
        );
        let mut diags = Vec::new();
        let expanded = expand_text(
            "EXPAND BRANCH a\nEXPAND BRANCH b\n",
            &defs,
            0,
            32,
            &mut diags,
        );
        assert!(diags.is_empty(), "{diags:?}");
        assert!(expanded.contains("L_OK_a__m1:"));
        assert!(expanded.contains("L_OK_b__m2:"));
        assert!(expanded.contains("jmp L_BAD_a__m1"));
        assert!(expanded.contains("L_BAD_b__m2:"));
        assert!(!expanded.contains("EXPAND"));
    }
}
