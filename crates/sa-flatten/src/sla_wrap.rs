use std::path::Path;

pub fn is_sla_wrap(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("sla") {
        return false;
    }
    let parts: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    parts
        .windows(2)
        .any(|w| w[0] == "sla_std" && w[1] == "wrap")
}

fn brace_delta(line: &str) -> i32 {
    line.matches('{').count() as i32 - line.matches('}').count() as i32
}

fn split_args(text: &str, caps: &[(String, String)]) -> String {
    text.split(',')
        .filter_map(|part| {
            let arg = part.trim();
            if arg.is_empty() {
                return None;
            }
            let prefix = caps
                .iter()
                .find(|(name, _)| name == arg)
                .map(|(_, prefix)| prefix.as_str())
                .unwrap_or("");
            Some(format!("{prefix}{arg}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn call_expr(text: &str) -> Option<(&str, &str)> {
    let lp = text.find('(')?;
    if !text[..lp].chars().all(|c| c.is_alphanumeric() || c == '_') || text[..lp].is_empty() {
        return None;
    }
    let rp = text.rfind(')')?;
    if rp <= lp || !text[rp + 1..].trim().is_empty() {
        return None;
    }
    Some((&text[..lp], &text[lp + 1..rp]))
}

pub fn adapt(content: &str) -> String {
    let mut out = String::new();
    let mut skip_depth: i32 = 0;
    let mut in_fn = false;
    let mut caps: Vec<(String, String)> = Vec::new();
    let mut tmp = 0usize;

    for raw in content.lines() {
        let line = raw.trim();

        if skip_depth > 0 {
            skip_depth += brace_delta(line);
            continue;
        }
        if (line.starts_with("struct ") || line.starts_with("impl ")) && line.ends_with('{') {
            skip_depth += brace_delta(line);
            continue;
        }
        if line.starts_with("fn ") && line.ends_with('{') {
            let header = line[3..line.len() - 1].trim();
            if let (Some(lp), Some(rp)) = (header.find('('), header.rfind(')')) {
                if rp > lp {
                    let name = header[..lp].trim();
                    let params = header[lp + 1..rp].trim();
                    let ret = header[rp + 1..]
                        .trim()
                        .strip_prefix("->")
                        .map(|s| s.trim())
                        .unwrap_or("void");
                    caps.clear();
                    for param in params.split(',') {
                        let mut rest = param.trim();
                        let mut prefix = String::new();
                        while let Some(first) = rest.chars().next() {
                            if matches!(first, '&' | '^' | '*') {
                                prefix.push(first);
                                rest = &rest[first.len_utf8()..];
                            } else {
                                break;
                            }
                        }
                        let arg = rest.split(':').next().unwrap_or("").trim();
                        if !arg.is_empty() {
                            caps.push((arg.to_string(), prefix));
                        }
                    }
                    out.push_str(&format!("@export {name}({params}) -> {ret}:\n"));
                    out.push_str("L_ENTRY:\n");
                    in_fn = true;
                    continue;
                }
            }
        }
        if line == "}" && in_fn {
            in_fn = false;
            continue;
        }
        if in_fn {
            if let Some(expr) = line.strip_prefix("return ").and_then(|s| s.strip_suffix(';')) {
                let expr = expr.trim();
                if let Some((callee, args)) = call_expr(expr) {
                    tmp += 1;
                    let reg = format!("w{tmp}");
                    out.push_str(&format!(
                        "    {reg} = call @{callee}({})\n",
                        split_args(args, &caps)
                    ));
                    out.push_str(&format!("    return {reg}\n"));
                } else {
                    out.push_str(&format!("    return {expr}\n"));
                }
                continue;
            }
            if let Some(stmt) = line.strip_suffix(';') {
                if let Some((callee, args)) = call_expr(stmt.trim()) {
                    out.push_str(&format!(
                        "    call @{callee}({})\n",
                        split_args(args, &caps)
                    ));
                    continue;
                }
            }
        }
        out.push_str(raw);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapts_a_thin_wrapper() {
        let src = "@import \"sa_std/math.sai\"\n\nfn f(&out: u64) -> i32 {\n    return g(out);\n}\n";
        let adapted = adapt(src);
        assert!(adapted.contains("@export f(&out: u64) -> i32:"));
        assert!(adapted.contains("w1 = call @g(&out)"));
        assert!(adapted.contains("return w1"));
    }

    #[test]
    fn skips_struct_and_impl_blocks() {
        let src = "struct S {\n    value: i64,\n}\nimpl S {\n    fn get(self) -> i64 {\n        return self.value;\n    }\n}\n";
        let adapted = adapt(src);
        assert!(!adapted.contains("fn get"));
        assert!(!adapted.contains("value: i64"));
    }

    #[test]
    fn detects_only_sla_std_wrap_paths() {
        assert!(is_sla_wrap(Path::new("/repo/sla_std/wrap/math.sla")));
        assert!(is_sla_wrap(Path::new("sla_std/wrap/io/print.sla")));
        assert!(!is_sla_wrap(Path::new("sla_std/cmp.sla")));
        assert!(!is_sla_wrap(Path::new("sa_std/math.sai")));
    }
}
