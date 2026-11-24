const RESERVED: &[&str] = &[
    "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete", "do",
    "else", "enum", "export", "extends", "false", "finally", "for", "function", "if", "import",
    "in", "instanceof", "new", "null", "return", "super", "switch", "this", "throw", "true",
    "try", "typeof", "var", "void", "while", "with", "yield", "let", "static", "await",
    "implements", "interface", "package", "private", "protected", "public", "arguments", "eval",
];

pub fn js_ident(name: &str) -> String {
    if name.is_empty() {
        return "_anon".to_string();
    }
    let mut out = String::with_capacity(name.len() + 2);
    for (i, c) in name.chars().enumerate() {
        let ok = c.is_ascii_alphanumeric() || c == '_' || c == '$';
        if i == 0 {
            if c.is_ascii_alphabetic() || c == '_' || c == '$' {
                out.push(c);
            } else {
                out.push('_');
                if ok {
                    out.push(c);
                }
            }
        } else if ok {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if RESERVED.contains(&name) {
        out.push('$');
    }
    if is_emitter_reserved(name) {
        out.push('$');
    }
    out
}

fn is_emitter_reserved(name: &str) -> bool {
    if name.starts_with("__sa") || name.starts_with("__SA") {
        return true;
    }
    name.len() >= 2 && name.starts_with('r') && name[1..].chars().all(|c| c.is_ascii_digit())
}

pub fn js_func_name(name: &str) -> String {
    let mut n = name;
    if n.len() >= 2 && n.starts_with('"') && n.ends_with('"') {
        n = &n[1..n.len() - 1];
    }
    if let Some(rest) = n.strip_prefix('@') {
        n = rest;
    }
    let base = js_ident(n);
    if base == "call" || base.starts_with("sa_") || base == "memory" || base == "SA_FUNCS" {
        format!("sa_fn_{base}")
    } else {
        base
    }
}

pub fn const_name(name: &str) -> String {
    format!("C_{}", js_ident(name))
}

pub fn js_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn js_int(v: i128) -> String {
    if v >= i32::MIN as i128 && v <= i32::MAX as i128 {
        format!("{v}")
    } else {
        format!("{v}n")
    }
}

pub fn js_float(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v == f64::INFINITY {
        return "Infinity".to_string();
    }
    if v == f64::NEG_INFINITY {
        return "-Infinity".to_string();
    }
    if v == 0.0 && v.is_sign_negative() {
        return "-0".to_string();
    }
    let s = format!("{v:?}");
    if s.ends_with(".0") {
        s[..s.len() - 2].to_string()
    } else {
        s
    }
}
