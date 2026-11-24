use sa_ir::Diagnostic;

pub fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    for c in input.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out
}

pub fn string(input: &str) -> String {
    format!("\"{}\"", escape(input))
}

pub fn diagnostics_json(diags: &[Diagnostic]) -> String {
    let mut items = Vec::with_capacity(diags.len());
    for d in diags {
        items.push(format!(
            "{{\"trap\":{},\"code\":{},\"loc\":{},\"message\":{}}}",
            string(d.trap.name()),
            d.code(),
            string(&d.loc.to_string()),
            string(&d.message)
        ));
    }
    format!("{{\"ok\":false,\"diagnostics\":[{}]}}", items.join(","))
}

pub fn check_json(functions: usize, instructions: usize, gas: u64) -> String {
    format!(
        "{{\"ok\":true,\"functions\":{functions},\"instructions\":{instructions},\"gas_used\":{gas}}}"
    )
}

pub fn tokens_json(tokens: usize, functions: usize, instructions: usize) -> String {
    format!("{{\"tokens\":{tokens},\"functions\":{functions},\"instructions\":{instructions}}}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::SourceLoc;

    #[test]
    fn escapes_control_characters() {
        assert_eq!(escape("a\"b\\c\nd\te"), "a\\\"b\\\\c\\nd\\te");
        assert_eq!(escape("\u{01}"), "\\u0001");
        assert_eq!(string("x\ry"), "\"x\\ry\"");
    }

    #[test]
    fn diagnostic_shape() {
        let d = Diagnostic::new(
            sa_ir::Trap::UseAfterMove,
            "bad \"move\"",
            SourceLoc::new("main.sa", 3, 7),
        );
        let text = diagnostics_json(&[d]);
        assert!(text.contains("\"trap\":\"UseAfterMove\""));
        assert!(text.contains("\"code\":1009"));
        assert!(text.contains("\"loc\":\"main.sa:3:7\""));
        assert!(text.contains("bad \\\"move\\\""));
        assert!(text.starts_with("{\"ok\":false,\"diagnostics\":["));
    }

    #[test]
    fn check_and_token_shapes() {
        assert_eq!(
            check_json(1, 2, 3),
            "{\"ok\":true,\"functions\":1,\"instructions\":2,\"gas_used\":3}"
        );
        assert_eq!(
            tokens_json(4, 5, 6),
            "{\"tokens\":4,\"functions\":5,\"instructions\":6}"
        );
    }
}
