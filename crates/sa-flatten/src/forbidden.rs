use sa_ir::{Diagnostic, Program, Trap};

const PATTERNS: &[&str] = &[
    ";", "{", "}", "=>", "let ", "fn ", "impl ", "struct ", "enum ", "match ", "while (", "for (",
    "if (",
];

pub(crate) fn check(program: &Program, diags: &mut Vec<Diagnostic>) {
    for function in &program.functions {
        for block in &function.blocks {
            for instruction in &block.instrs {
                if let Some(pattern) = find_forbidden(&instruction.raw) {
                    let message = format!(
                        "forbidden syntax '{}' is not valid SA",
                        pattern.trim_end()
                    );
                    let diagnostic = Diagnostic::new(
                        Trap::ForbiddenSyntax,
                        message,
                        instruction.loc.clone(),
                    )
                    .in_function(function.sig.name.clone());
                    if !diags.contains(&diagnostic) {
                        diags.push(diagnostic);
                    }
                }
            }
        }
    }
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn find_forbidden(raw: &str) -> Option<&'static str> {
    let chars: Vec<char> = raw.chars().collect();
    let mut in_string = false;
    let mut escaped = false;
    for i in 0..chars.len() {
        let c = chars[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            continue;
        }
        for pattern in PATTERNS {
            let width = pattern.chars().count();
            if i + width > chars.len() {
                continue;
            }
            let mut matches = true;
            for (offset, expected) in pattern.chars().enumerate() {
                if chars[i + offset] != expected {
                    matches = false;
                    break;
                }
            }
            if !matches {
                continue;
            }
            if *pattern == "fn " {
                let mut next = i + 3;
                while next < chars.len() && chars[next] == ' ' {
                    next += 1;
                }
                let name_start = next;
                while next < chars.len() && is_word_char(chars[next]) {
                    next += 1;
                }
                if next == name_start || next >= chars.len() || chars[next] != '(' {
                    continue;
                }
            }
            let starts_word = pattern
                .chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false);
            let ends_word = pattern
                .chars()
                .last()
                .map(|c| c.is_ascii_alphanumeric())
                .unwrap_or(false);
            let prev_ok = !starts_word
                || i == 0
                || !is_word_char(chars[i - 1]);
            let next_pos = i + width;
            let next_ok = !ends_word
                || next_pos >= chars.len()
                || !is_word_char(chars[next_pos]);
            if prev_ok && next_ok {
                return Some(pattern);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{
        Block, Function, Instruction, InstructionKind, Signature, SourceLoc,
    };

    fn program_with_raw(raw: &str) -> Program {
        let loc = SourceLoc::new("test.sa", 3, 5);
        let mut block = Block::new("entry", loc.clone());
        block
            .instrs
            .push(Instruction::new(InstructionKind::Nop, loc.clone(), raw));
        let mut function = Function::new(Signature::new("main"), loc);
        function.blocks.push(block);
        let mut program = Program::new();
        program.functions.push(function);
        program
    }

    #[test]
    fn rejects_c_like_constructs() {
        for raw in [
            "x = 1;",
            "x = { 1 }",
            "x => y",
            "let x = 1",
            "fn foo() {}",
            "impl Foo",
            "struct Foo",
            "enum E",
            "match x",
            "while (x) {}",
            "for (x) {}",
            "if (x) {}",
        ] {
            let mut diags = Vec::new();
            check(&program_with_raw(raw), &mut diags);
            assert_eq!(diags.len(), 1, "expected rejection for '{}'", raw);
            assert_eq!(diags[0].trap, Trap::ForbiddenSyntax);
            assert_eq!(diags[0].loc.file, "test.sa");
            assert_eq!(diags[0].function.as_deref(), Some("main"));
        }
    }

    #[test]
    fn allows_branch_arrow_and_string_contents() {
        for raw in ["br c -> L1, L2", "x = add r0, utf8:\"let x; if (y)\""] {
            let mut diags = Vec::new();
            check(&program_with_raw(raw), &mut diags);
            assert!(diags.is_empty(), "unexpected rejection for '{}'", raw);
        }
    }
}
